//! 校准页：官方 15 个布局参数覆盖不到的机型，在这里补分区与面板参数。
//!
//! 两段配置各管一摊：`partition` 决定网格切在哪、切几份；`calibration` 决定面板的
//! 绝对坐标范围与分辨率报的是什么。前者影响按键落在哪一格，后者影响毫米级阈值
//! （手势距离、NumLock 有效距离）算出来的像素值对不对。
//!
//! 采样本身要在真机上按住触摸板，界面里做不了；所以这页的定位是「把采集到的样本
//! 喂进来解算，或者手工填」，解算用与 CLI 完全同一个纯函数。

use eframe::egui;

use asus_numpad::partition::{
    solve_calibration, CornerSamples, GridProbe, RawSamples,
};

use super::calib_view;
use super::extend::save_snippet;
use super::{Action, App};
use crate::theme;

impl App {
    /// 校准页整体。与扩展页同一套「画完比一次配置」的标脏逻辑。
    pub(super) fn draw_calib(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let before = self.driver.cfg.clone();

        self.import_dropped_samples(ui);
        self.refresh_calib_probes();
        self.draw_calib_header(ui, action);
        ui.add_space(10.0);
        self.draw_calib_partition(ui);
        ui.add_space(10.0);
        self.draw_calib_axes(ui);
        ui.add_space(10.0);
        self.draw_calib_samples(ui);
        ui.add_space(10.0);
        self.draw_calib_snippet(ui);

        if self.driver.cfg != before {
            self.driver.dirty = true;
        }
    }

    /// 页首：写哪份配置、改没改、存盘动作用。
    fn draw_calib_header(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let dirty = self.driver.dirty;
        let write_path = self.driver.write_path.display().to_string();

        theme::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                theme::title_row_help(
                    ui,
                    &p,
                    "自定义分区与校准",
                    Some(
                        "给官方表里没有的机型准备的：贴纸行列不是 5x4、列宽不等分、\
                         或者内核上报的 ABS_MT_POSITION 范围不准时，在这里改。",
                    ),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if dirty {
                        theme::pill(ui, "有改动未保存", p.warn)
                            .on_hover_text("点「保存」或「保存并重启服务」落盘");
                    } else {
                        theme::pill(ui, "已与磁盘一致", p.ok);
                    }
                });
            });
            ui.add_space(2.0);
            theme::note(ui, &p, format!("写入：{write_path}"));
            ui.add_space(6.0);
            calib_view::status_row(ui, &p, &self.driver.cfg);
            ui.add_space(8.0);
            theme::ctrl_column(ui, |ui| {
                ui.horizontal(|ui| {
                    let resp = theme::primary(ui, &p, "保存");
                    if resp.clicked() {
                        *action = Some(Action::SaveDriver);
                    }
                    let resp = theme::secondary(ui, &p, "保存并重启服务");
                    if resp.clicked() {
                        *action = Some(Action::SaveDriverAndRestart);
                    }
                    let resp = theme::secondary(ui, &p, "重新读取");
                    if resp.clicked() {
                        *action = Some(Action::ReloadDriver);
                    }
                });
            });
            ui.add_space(4.0);
            theme::note(
                ui,
                &p,
                "分区只改几何：键表仍由「扩展」页的自定义布局给。两段都留空时，官方参数原样生效。",
            );
        });
    }

    /// 卡一：分区矩形与行列。
    fn draw_calib_partition(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "分区",
                Some("直接用边界把网格切成任意行列。坐标是触摸板的原始值，与 evtest 里看到的一致。"),
            );
            ui.add_space(4.0);

            theme::row_hint(
                ui,
                &p,
                "生效布局",
                Some("auto = 当前生效布局；也可以填具体布局名或自定义布局名。"),
                "auto / 布局名",
                |ui| theme::text_field(ui, &mut self.driver.cfg.partition.layout, "auto"),
            );

            // 矩形：勾上才参与，四值按 x0 y0 x1 y1 排
            let mut has_rect = self.driver.cfg.partition.rect.is_some();
            let resp = theme::check_row(
                ui,
                &p,
                &mut has_rect,
                "自定义矩形",
                Some("不勾就用官方 index0/index4/level0/level3 推出来的网格。"),
            );
            if resp.changed() {
                self.driver.cfg.partition.rect = if has_rect {
                    Some([0.0, 0.0, 3600.0, 2200.0])
                } else {
                    None
                };
            }
            if let Some(rect) = self.driver.cfg.partition.rect {
                // 四个角拆成独立变量再写回：同一张卡里两个闭包同时借一个数组的下标，
                // borrow checker 会判定成整体借用。
                let (mut x0, mut y0, mut x1, mut y1) = (rect[0], rect[1], rect[2], rect[3]);
                theme::row_hint(
                    ui,
                    &p,
                    "左上 x / y",
                    Some("网格左上角在触摸板坐标系里的位置。"),
                    "原始值",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, &mut x0, -100000.0..=100000.0),
                            |ui| theme::pair_num(ui, &mut y0, -100000.0..=100000.0),
                        )
                    },
                );
                theme::row_hint(
                    ui,
                    &p,
                    "右下 x / y",
                    Some("网格右下角；必须比左上角大，否则这条配置会被拒绝。"),
                    "原始值",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, &mut x1, -100000.0..=100000.0),
                            |ui| theme::pair_num(ui, &mut y1, -100000.0..=100000.0),
                        )
                    },
                );
                if [x0, y0, x1, y1] != rect {
                    self.driver.cfg.partition.rect = Some([x0, y0, x1, y1]);
                }
            }

            // 预览：把当前配置直接画出来。四个角、行列数、权重、NumLock 格填完长什么样，
            // 只有画出来才能判断对不对——数字全对而网格偏一格，是这页最容易犯的错。
            theme::block(
                ui,
                &p,
                "预览",
                Some(
                    "按当前配置画出的网格，鼠标移上去读每格中心坐标；样本点也画在这张图上。\
                     底下一整块是触摸板，网格之外就是光标区。",
                ),
                |ui| {
                    let (hover, _err) = calib_view::plate(
                        ui,
                        &p,
                        &self.driver.cfg,
                        &self.calib_probes,
                    );
                    if !self.calib_probes.is_empty() {
                        let at = self.calib_probes.len();
                        ui.label(
                            egui::RichText::new(format!("已画出 {at} 个样本点"))
                                .size(11.5)
                                .color(p.text_weak),
                        );
                    }
                    // 读数放在图下面一行：整宽铺开，悬停时换内容而不改行高，图不会跟着跳
                    let grid = calib_view::resolve(&self.driver.cfg).ok();
                    let (text, color) = calib_view::readout(&p, grid.as_ref(), hover.as_ref());
                    ui.label(egui::RichText::new(text).size(11.5).color(color));
                },
            );

            // 行列：留空跟随键表
            let mut follow_rows = self.driver.cfg.partition.rows.is_none();
            let resp = theme::check_row(
                ui,
                &p,
                &mut follow_rows,
                "行列跟随键表",
                Some("键表本身就有行列（4 行、1..=8 列），一般不用在这里另填。"),
            );
            if resp.changed() {
                let (rows, cols) = if follow_rows {
                    (None, None)
                } else {
                    (Some(4usize), Some(5usize))
                };
                self.driver.cfg.partition.rows = rows;
                self.driver.cfg.partition.cols = cols;
            }
            if let (Some(rows), Some(cols)) = (
                self.driver.cfg.partition.rows.as_mut(),
                self.driver.cfg.partition.cols.as_mut(),
            ) {
                theme::row_hint(
                    ui,
                    &p,
                    "列数 / 行数",
                    Some("列数可以 1..=8；行数固定 4（键表就是 4 行）。"),
                    "列 / 行",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, cols, 1..=8),
                            |ui| theme::pair_num(ui, rows, 1..=4),
                        )
                    },
                );
            }

            theme::check_row(
                ui,
                &p,
                &mut self.driver.cfg.partition.snap_outside,
                "网格外吸附到最近一格",
                Some("默认关闭：网格外的落点属于光标区，按官方行为不产生按键。"),
            );

            // 列宽 / 行高权重。改列数行数后长度自动补齐，不然一改列数整段就被判成坏值。
            let keys = {
                let table = self.driver.cfg.key_table();
                (table.cols().max(1), table.rows().len().max(1))
            };
            let cols_now = self.driver.cfg.partition.cols.unwrap_or(keys.0).clamp(1, 8);
            let rows_now = self.driver.cfg.partition.rows.unwrap_or(keys.1).clamp(1, 4);
            if !self.driver.cfg.partition.col_ratios.is_empty()
                && self.driver.cfg.partition.col_ratios.len() != cols_now
            {
                self.driver.cfg.partition.col_ratios.resize(cols_now, 1.0);
            }
            if !self.driver.cfg.partition.row_ratios.is_empty()
                && self.driver.cfg.partition.row_ratios.len() != rows_now
            {
                self.driver.cfg.partition.row_ratios.resize(rows_now, 1.0);
            }

            let mut col_weights = !self.driver.cfg.partition.col_ratios.is_empty();
            let resp = theme::check_row(
                ui,
                &p,
                &mut col_weights,
                "列宽不等分",
                Some("勾上按权重分配列宽，权重自动归一；全填 1 就是等分。"),
            );
            if resp.changed() {
                self.driver.cfg.partition.col_ratios = if col_weights {
                    vec![1.0; cols_now]
                } else {
                    Vec::new()
                };
            }
            if !self.driver.cfg.partition.col_ratios.is_empty() {
                theme::block(
                    ui,
                    &p,
                    "列宽权重",
                    Some("从左到右每一列占几份；数字越大这列越宽。"),
                    |ui| weight_row(ui, &mut self.driver.cfg.partition.col_ratios),
                );
            }

            let mut row_weights = !self.driver.cfg.partition.row_ratios.is_empty();
            let resp = theme::check_row(
                ui,
                &p,
                &mut row_weights,
                "行高不等分",
                Some("同上，按行分配高度；触摸板上下留白不等时用它。"),
            );
            if resp.changed() {
                self.driver.cfg.partition.row_ratios = if row_weights {
                    vec![1.0; rows_now]
                } else {
                    Vec::new()
                };
            }
            if !self.driver.cfg.partition.row_ratios.is_empty() {
                theme::block(
                    ui,
                    &p,
                    "行高权重",
                    Some("从上到下每一行占几份；数字越大这行越高。"),
                    |ui| weight_row(ui, &mut self.driver.cfg.partition.row_ratios),
                );
            }

            let mut nl_custom = self.driver.cfg.partition.numlock_cell.is_some();
            let resp = theme::check_row(
                ui,
                &p,
                &mut nl_custom,
                "指定 NumLock 格",
                Some("不勾就是第 1 行最后一列，也就是官方布局里那个位置。"),
            );
            if resp.changed() {
                self.driver.cfg.partition.numlock_cell = if nl_custom {
                    Some([0, cols_now.saturating_sub(1)])
                } else {
                    None
                };
            }
            if let Some([row, col]) = self.driver.cfg.partition.numlock_cell {
                let (mut row, mut col) = (row, col);
                theme::row_hint(
                    ui,
                    &p,
                    "NumLock 行 / 列",
                    Some("行列都从 0 数起；预览图上这一格标着 NL。"),
                    "行 / 列",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, &mut row, 0..=rows_now.saturating_sub(1)),
                            |ui| theme::pair_num(ui, &mut col, 0..=cols_now.saturating_sub(1)),
                        )
                    },
                );
                if [row, col] != self.driver.cfg.partition.numlock_cell.unwrap_or([row, col]) {
                    self.driver.cfg.partition.numlock_cell = Some([row, col]);
                }
            }

            ui.add_space(4.0);
            calib_view::lint_notes(ui, &p, &self.driver.cfg);
        });
    }

    /// 卡二：轴范围与分辨率。
    fn draw_calib_axes(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "手动校准",
                Some(
                    "内核报的 ABS_MT_POSITION_* 范围或分辨率不准时，用这里覆盖。只做线性映射，\
                     不交换轴也不翻转——真机上极少见，改错了比不改更糟。",
                ),
            );
            ui.add_space(4.0);

            // X 轴
            let mut x_on = !self.driver.cfg.calibration.x.is_unset();
            let resp = theme::check_row(
                ui,
                &p,
                &mut x_on,
                "覆盖 X 轴",
                Some("不勾就沿用内核上报的范围与分辨率。"),
            );
            if resp.changed() {
                self.driver.cfg.calibration.x = if x_on {
                    asus_numpad::partition::AxisCal {
                        min: Some(0),
                        max: Some(3600),
                        units_per_mm: None,
                    }
                } else {
                    Default::default()
                };
            }
            if !self.driver.cfg.calibration.x.is_unset() {
                let axis = &mut self.driver.cfg.calibration.x;
                theme::row_hint(
                    ui,
                    &p,
                    "X 范围",
                    Some("ABS_MT_POSITION_X 的最小值与最大值。"),
                    "原始值",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, axis.min.get_or_insert(0), -100000..=100000),
                            |ui| {
                                theme::pair_num(ui, axis.max.get_or_insert(3600), -100000..=100000)
                            },
                        )
                    },
                );
                theme::row_hint(
                    ui,
                    &p,
                    "X 分辨率",
                    Some("每毫米多少个单位，等同 evdev 的 resolution；留空沿用内核值。"),
                    "units/mm",
                    |ui| {
                        let mut on = axis.units_per_mm.is_some();
                        let resp = theme::switch(ui, &p, &mut on);
                        if resp.changed() {
                            axis.units_per_mm = if on { Some(40.0) } else { None };
                        }
                        if let Some(units) = axis.units_per_mm.as_mut() {
                            theme::pair_num(ui, units, 1.0..=400.0);
                        }
                        resp
                    },
                );
            }

            ui.add_space(4.0);

            // Y 轴
            let mut y_on = !self.driver.cfg.calibration.y.is_unset();
            let resp = theme::check_row(
                ui,
                &p,
                &mut y_on,
                "覆盖 Y 轴",
                Some("与 X 轴同理，两轴可以只覆盖一个。"),
            );
            if resp.changed() {
                self.driver.cfg.calibration.y = if y_on {
                    asus_numpad::partition::AxisCal {
                        min: Some(0),
                        max: Some(2200),
                        units_per_mm: None,
                    }
                } else {
                    Default::default()
                };
            }
            if !self.driver.cfg.calibration.y.is_unset() {
                let axis = &mut self.driver.cfg.calibration.y;
                theme::row_hint(
                    ui,
                    &p,
                    "Y 范围",
                    Some("ABS_MT_POSITION_Y 的最小值与最大值。"),
                    "原始值",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, axis.min.get_or_insert(0), -100000..=100000),
                            |ui| {
                                theme::pair_num(ui, axis.max.get_or_insert(2200), -100000..=100000)
                            },
                        )
                    },
                );
                theme::row_hint(
                    ui,
                    &p,
                    "Y 分辨率",
                    Some("每毫米多少个单位；留空沿用内核值。"),
                    "units/mm",
                    |ui| {
                        let mut on = axis.units_per_mm.is_some();
                        let resp = theme::switch(ui, &p, &mut on);
                        if resp.changed() {
                            axis.units_per_mm = if on { Some(40.0) } else { None };
                        }
                        if let Some(units) = axis.units_per_mm.as_mut() {
                            theme::pair_num(ui, units, 1.0..=400.0);
                        }
                        resp
                    },
                );
            }

            if !self.driver.cfg.calibration.is_unset() {
                ui.add_space(4.0);
                theme::note(
                    ui,
                    &p,
                    "下界必须小于上界；写反了或被填成 0 以下的分辨率，加载时会丢掉那一轴并在提示里点名。",
                );
            }
        });
    }

    /// 卡三：样本回放——把真机采到的落点喂进解算器，结果直接写回配置。
    fn draw_calib_samples(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "样本回放",
                Some(
                    "采集要在真机上按住触摸板，界面里做不了：在终端跑 `asus-numpad calibrate --samples s.json` \
                     采一份，或者手写一份样本文件，回到这里解算。",
                ),
            );
            ui.add_space(4.0);

            theme::row(
                ui,
                &p,
                "样本文件",
                Some("JSON：corners 四角 + first_point / last_point 两个对角格中心。"),
                |ui| {
                    theme::text_field(
                        ui,
                        &mut self.calib_samples_path,
                        "/path/to/samples.json",
                    )
                },
            );

            let mut cols = self.calib_cols;
            let mut rows = self.calib_rows;
            theme::row_hint(
                ui,
                &p,
                "列数 / 行数",
                Some("解算要知道样本对应几列几行；与键表一致，否则格宽会算错。"),
                "列 / 行",
                |ui| {
                    theme::pair(
                        ui,
                        |ui| theme::pair_num(ui, &mut cols, 1..=8),
                        |ui| theme::pair_num(ui, &mut rows, 1..=4),
                    )
                },
            );
            self.calib_cols = cols;
            self.calib_rows = rows;

            ui.add_space(2.0);
            theme::ctrl_column(ui, |ui| {
                ui.horizontal(|ui| {
                    let resp = theme::secondary(ui, &p, "读取并解算");
                    let resp = theme::tip(
                        resp,
                        "读样本文件，解出网格矩形与格尺寸，直接写进上面的分区。",
                    );
                    if resp.clicked() {
                        self.calib_msg = Some(self.run_calibration());
                    }
                    let resp = theme::secondary(ui, &p, "清空分区");
                    let resp = theme::tip(resp, "把分区整段清掉，回到官方参数。");
                    if resp.clicked() {
                        self.driver.cfg.partition = Default::default();
                        self.calib_msg = Some(("已清空分区".to_string(), false));
                    }
                });
            });

            if let Some((text, bad)) = self.calib_msg.clone() {
                ui.add_space(4.0);
                if bad {
                    theme::note(ui, &p, format!("解算失败：{text}"));
                } else {
                    for line in text.lines() {
                        theme::note(ui, &p, line.to_string());
                    }
                }
            }

            // 解算只保证矩形对；样本点究竟落进哪一格，还得单独核一遍。
            // 点在手边时最常犯的错就是「按到了隔壁格」，数字全对、键全错一格。
            if let Some(err) = self.calib_probe_msg.clone() {
                ui.add_space(4.0);
                theme::note(ui, &p, format!("样本没读成：{err}（预览图上暂时没有样本点）"));
            } else if !self.calib_probes.is_empty() {
                if let Ok(grid) = calib_view::resolve(&self.driver.cfg) {
                    let issues = calib_view::probe_check(&grid, &self.calib_probes);
                    ui.add_space(4.0);
                    if issues.is_empty() {
                        theme::note(ui, &p, "样本点都和网格对得上：每个点都落在它该在的那一格。");
                    } else {
                        for line in issues {
                            theme::note(ui, &p, line);
                        }
                    }
                }
            }
        });
    }

    /// 卡四：把分区 + 校准打包成片段，同机型对拷用。
    fn draw_calib_snippet(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "配置片段",
                Some(
                    "把 partition 与 calibration 打包成一段 JSON。不含键表：\
                     键表在「扩展」页的自定义布局里。",
                ),
            );
            ui.add_space(4.0);
            theme::row(
                ui,
                &p,
                "片段",
                Some("点「导出」从当前配置生成；也可以直接粘贴别人给的片段。"),
                |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.calib_snippet)
                            .desired_rows(5)
                            .desired_width(theme::ctrl_width())
                            .hint_text("{ ... }"),
                    )
                },
            );
            ui.add_space(2.0);
            theme::ctrl_column(ui, |ui| {
                ui.horizontal(|ui| {
                    let resp = theme::secondary(ui, &p, "导出");
                    if resp.clicked() {
                        self.calib_snippet = calib_fragment_of(&self.driver.cfg);
                        let len = self.calib_snippet.len();
                        self.calib_snippet_msg =
                            Some((format!("已生成 {len} 字节的片段"), false));
                    }
                    let resp = theme::secondary(ui, &p, "复制");
                    if resp.clicked() {
                        if self.calib_snippet.trim().is_empty() {
                            self.calib_snippet_msg =
                                Some(("框里还是空的，先点「导出」".to_string(), true));
                        } else {
                            ui.ctx().copy_text(self.calib_snippet.clone());
                            let len = self.calib_snippet.len();
                            self.calib_snippet_msg =
                                Some((format!("已复制 {len} 字节到剪贴板"), false));
                        }
                    }
                    let resp = theme::secondary(ui, &p, "存成文件");
                    if resp.clicked() {
                        self.calib_snippet_msg =
                            Some(match save_snippet(&self.calib_snippet) {
                                Ok(path) => (format!("已写入 {}", path.display()), false),
                                Err(err) => (err, true),
                            });
                    }
                    let resp = theme::secondary(ui, &p, "应用");
                    let resp = theme::tip(
                        resp,
                        "写进当前配置；只有 partition / calibration 两段，其它键保持原样。",
                    );
                    if resp.clicked() {
                        self.calib_snippet_msg = Some(apply_calib_fragment(
                            &mut self.driver.cfg,
                            &self.calib_snippet,
                        ));
                    }
                });
            });
            if let Some((text, bad)) = self.calib_snippet_msg.clone() {
                ui.add_space(4.0);
                if bad {
                    theme::note(ui, &p, format!("片段有问题：{text}"));
                } else {
                    theme::note(ui, &p, text);
                }
            }
        });
    }

    /// 读样本文件 → 解算 → 写回分区。返回给界面显示的文案与是否出错。
    fn run_calibration(&mut self) -> (String, bool) {
        let path = self.calib_samples_path.trim().to_string();
        if path.is_empty() {
            return ("先填样本文件路径".to_string(), true);
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => return (format!("读不到 {path}：{err}"), true),
        };
        let samples = match parse_samples(&text) {
            Ok(samples) => samples,
            Err(err) => return (err, true),
        };
        let cols = self.calib_cols.clamp(1, 8);
        let rows = self.calib_rows.clamp(1, 4);
        let probe = GridProbe {
            first_cell: (0, 0),
            first_point: samples.first_point,
            last_cell: (rows - 1, cols - 1),
            last_point: samples.last_point,
            cols,
            rows,
        };
        let cal = match solve_calibration(&samples.corners, &probe) {
            Ok(cal) => cal,
            Err(err) => return (err, true),
        };

        let mut out = String::new();
        out.push_str(&format!(
            "网格 {}列 x {}行  [{}] .. [{}]",
            cal.partition.cols.unwrap_or(cols),
            cal.partition.rows.unwrap_or(rows),
            cal.partition
                .rect
                .map(|r| format!("{:.0}, {:.0}", r[0], r[1]))
                .unwrap_or_default(),
            cal.partition
                .rect
                .map(|r| format!("{:.0}, {:.0}", r[2], r[3]))
                .unwrap_or_default(),
        ));
        if let Some(rect) = cal.partition.rect {
            let w = (rect[2] - rect[0]) / cols as f64;
            let h = (rect[3] - rect[1]) / rows as f64;
            out.push('\n');
            out.push_str(&format!("格尺寸 {w:.1} x {h:.1} units（已写进分区）"));
        }
        out.push('\n');
        out.push_str(&format!(
            "实测角点范围 x {}..{} y {}..{}——若与内核上报差很多，再在「手动校准」里覆盖轴范围。",
            cal.panel.xmin, cal.panel.xmax, cal.panel.ymin, cal.panel.ymax
        ));

        self.driver.cfg.partition = cal.partition;
        self.calib_probes = probes_of(&samples, cols, rows);
        (out, false)
    }

    /// 拖进来的文件直接当样本路径：省掉「翻目录找文件」这一步。
    fn import_dropped_samples(&mut self, ui: &mut egui::Ui) {
        let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        let Some(path) = dropped.into_iter().next() else {
            return;
        };
        let text = path.display().to_string();
        if text == self.calib_samples_path {
            return;
        }
        self.calib_samples_path = text.clone();
        self.calib_msg = Some((format!("已载入样本文件 {text}"), false));
    }

    /// 样本路径或行列数一变就重解析：预览图上的点跟着输入框走，不必再点「读取并解算」。
    ///
    /// 每帧只比一个字符串，没有 IO；真正读文件只发生在路径或行列数变了以后。
    fn refresh_calib_probes(&mut self) {
        let key = format!(
            "{}|{}x{}",
            self.calib_samples_path.trim(),
            self.calib_cols,
            self.calib_rows
        );
        if key == self.calib_probe_path {
            return;
        }
        self.calib_probe_path = key;
        self.calib_probes.clear();
        self.calib_probe_msg = None;
        let path = self.calib_samples_path.trim().to_string();
        if path.is_empty() {
            return;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                self.calib_probe_msg = Some(format!("读不到 {path}：{err}"));
                return;
            }
        };
        match parse_samples(&text) {
            Ok(samples) => self.calib_probes = probes_of(&samples, self.calib_cols, self.calib_rows),
            Err(err) => self.calib_probe_msg = Some(err),
        }
    }
}

/// 样本 JSON → 结构化样本；字段缺失或类型不对时把是哪个键说清楚。
fn parse_samples(text: &str) -> Result<RawSamples, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("不是合法 JSON：{err}"))?;
    let corner = |name: &str| -> Result<(i32, i32), String> {
        let item = value
            .get("corners")
            .and_then(|c| c.get(name))
            .ok_or_else(|| format!("样本缺少 corners.{name}"))?;
        pair(item).ok_or_else(|| format!("corners.{name} 必须是 [x, y] 两个整数"))
    };
    let point = |name: &str| -> Result<(i32, i32), String> {
        let item = value
            .get(name)
            .ok_or_else(|| format!("样本缺少 {name}"))?;
        pair(item).ok_or_else(|| format!("{name} 必须是 [x, y] 两个整数"))
    };
    Ok(RawSamples {
        corners: CornerSamples {
            top_left: corner("top_left")?,
            top_right: corner("top_right")?,
            bottom_left: corner("bottom_left")?,
            bottom_right: corner("bottom_right")?,
        },
        first_point: point("first_point")?,
        last_point: point("last_point")?,
    })
}

fn pair(value: &serde_json::Value) -> Option<(i32, i32)> {
    let arr = value.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let x = arr[0].as_i64()?;
    let y = arr[1].as_i64()?;
    Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
}

/// 分区 + 校准打包成一段 JSON 文本。
fn calib_fragment_of(cfg: &asus_numpad::config::Config) -> String {
    let value = serde_json::json!({
        "partition": cfg.partition,
        "calibration": cfg.calibration,
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

/// 片段写回配置：两段先都解析成功，再一次性写回。
fn apply_calib_fragment(
    cfg: &mut asus_numpad::config::Config,
    text: &str,
) -> (String, bool) {
    let text = text.trim();
    if text.is_empty() {
        return ("片段是空的：先点「导出」，或粘贴别人的片段".to_string(), true);
    }
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(err) => return (format!("不是合法 JSON：{err}"), true),
    };
    let partition = match value.get("partition") {
        Some(raw) => match serde_json::from_value(raw.clone()) {
            Ok(spec) => Some(spec),
            Err(err) => return (format!("partition 解析失败：{err}"), true),
        },
        None => None,
    };
    let calibration = match value.get("calibration") {
        Some(raw) => match serde_json::from_value(raw.clone()) {
            Ok(spec) => Some(spec),
            Err(err) => return (format!("calibration 解析失败：{err}"), true),
        },
        None => None,
    };
    if partition.is_none() && calibration.is_none() {
        return (
            "片段里没有 partition 或 calibration，没什么可应用的".to_string(),
            true,
        );
    }
    let mut touched = Vec::new();
    if let Some(spec) = partition {
        cfg.partition = spec;
        touched.push("分区".to_string());
    }
    if let Some(spec) = calibration {
        cfg.calibration = spec;
        touched.push("校准".to_string());
    }
    (format!("已应用：{}", touched.join("、")), false)
}


/// 样本 → 预览图上的点。
///
/// 四角只画不判：它们标的是触摸板外框，网格可以只覆盖其中一块，落在网格外属正常。
/// 格中心那两个才带「该在第几格」，画上去用来自查有没有按偏。
fn probes_of(samples: &RawSamples, cols: usize, rows: usize) -> Vec<calib_view::Probe> {
    let cols = cols.clamp(1, 8);
    let rows = rows.clamp(1, 4);
    let probe = |raw: (i32, i32), want, label| calib_view::Probe {
        raw: (raw.0 as f64, raw.1 as f64),
        want,
        label,
    };
    vec![
        probe(samples.corners.top_left, None, "左上角"),
        probe(samples.corners.top_right, None, "右上角"),
        probe(samples.corners.bottom_left, None, "左下角"),
        probe(samples.corners.bottom_right, None, "右下角"),
        probe(samples.first_point, Some((0, 0)), "首格中心"),
        probe(samples.last_point, Some((rows - 1, cols - 1)), "末格中心"),
    ]
}

/// 一行并排的权重数字框：几份就几个，等宽平分控件列，右端与其它控件压同一条线。
fn weight_row(ui: &mut egui::Ui, values: &mut [f64]) {
    let count = values.len().max(1) as f32;
    let gap = ui.spacing().item_spacing.x;
    let width = ((theme::ctrl_width() - gap * (count - 1.0)) / count).max(40.0);
    ui.horizontal(|ui| {
        for (index, value) in values.iter_mut().enumerate() {
            let resp = ui.add_sized(
                egui::vec2(width, theme::CTRL_H),
                egui::DragValue::new(value)
                    .speed(0.05)
                    .range(0.05..=10.0)
                    .max_decimals(2),
            );
            theme::tip(resp, format!("第 {} 份的权重", index + 1));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_json() -> String {
        r#"{
            "corners": {
                "top_left": [405, 520],
                "top_right": [3630, 520],
                "bottom_left": [405, 2280],
                "bottom_right": [3630, 2280]
            },
            "first_point": [730, 740],
            "last_point": [3300, 2060]
        }"#
        .to_string()
    }

    #[test]
    fn fragment_roundtrip_keeps_both_sections() {
        let mut src = asus_numpad::config::Config::default();
        src.partition.rect = Some([100.0, 200.0, 3000.0, 2000.0]);
        src.partition.cols = Some(6);
        src.partition.rows = Some(4);
        src.calibration.x.min = Some(0);
        src.calibration.x.max = Some(3600);

        let text = calib_fragment_of(&src);
        let mut dst = asus_numpad::config::Config::default();
        let (msg, bad) = apply_calib_fragment(&mut dst, &text);
        assert!(!bad, "{msg}");
        assert_eq!(dst.partition, src.partition);
        assert_eq!(dst.calibration, src.calibration);
    }

    #[test]
    fn fragment_rejects_bad_payload() {
        let mut cfg = asus_numpad::config::Config::default();
        let (msg, bad) = apply_calib_fragment(&mut cfg, "not json");
        assert!(bad);
        assert!(msg.contains("JSON"), "{msg}");

        let (msg, bad) = apply_calib_fragment(&mut cfg, "{\"partition\": 5}");
        assert!(bad);
        assert!(msg.contains("partition"), "{msg}");

        let (msg, bad) = apply_calib_fragment(&mut cfg, "{\"other\": 1}");
        assert!(bad);
        assert!(msg.contains("没什么可应用"), "{msg}");
    }

    #[test]
    fn samples_missing_key_is_named() {
        let err = parse_samples("{\"corners\":{}}").unwrap_err();
        assert!(err.contains("corners.top_left"), "{err}");

        let err = parse_samples("{\"corners\":{\"top_left\":[1,2],\"top_right\":[3,4],\
             \"bottom_left\":[5,6],\"bottom_right\":[7,8]}}")
            .unwrap_err();
        assert!(err.contains("first_point"), "{err}");
    }

    /// 样本回放走的是与 CLI 同一个纯函数：把样本解出来的分区写回配置后，
    /// 键位图应当能对上自定义行列。
    #[test]
    fn solved_partition_covers_probe_cells() {
        let samples = parse_samples(&sample_json()).unwrap();
        let probe = GridProbe {
            first_cell: (0, 0),
            first_point: samples.first_point,
            last_cell: (3, 4),
            last_point: samples.last_point,
            cols: 5,
            rows: 4,
        };
        let cal = solve_calibration(&samples.corners, &probe).unwrap();
        let rect = cal.partition.rect.expect("解算结果必须带矩形");
        assert_eq!(cal.partition.cols, Some(5));
        assert_eq!(cal.partition.rows, Some(4));
        assert!(rect[0] < samples.first_point.0 as f64);
        assert!(rect[2] > samples.last_point.0 as f64);
        assert!(rect[1] < samples.first_point.1 as f64);
        assert!(rect[3] > samples.last_point.1 as f64);
    }
}
