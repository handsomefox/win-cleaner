//! The file preview: every file cleaning one target would move, in a table
//! that draws only the rows in view, so a target holding a million files
//! scrolls as smoothly as one holding ten.

use std::cmp::Reverse;
use std::path::{Path, PathBuf};

use cleaner_core::{FileEntry, Rules, human_bytes};
use eframe::egui::{self, RichText};
use egui_extras::{Column, TableBuilder};

use crate::strings::UiText;
use crate::theme;
use crate::worker::FILE_LIST_LIMIT;

/// Which column the table is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortBy {
    Path,
    Size,
    Modified,
}

enum Contents {
    Loading,
    Loaded {
        files: Vec<FileEntry>,
        truncated: bool,
        bytes: u64,
    },
}

pub(crate) struct FilesView {
    title: String,
    contents: Contents,
    sort: SortBy,
    descending: bool,
}

pub(crate) enum FilesAction {
    /// Add this path to the exclusions.
    Exclude(PathBuf),
    Close,
}

impl FilesView {
    pub(crate) fn new(title: String) -> Self {
        Self {
            title,
            contents: Contents::Loading,
            sort: SortBy::Size,
            descending: true,
        }
    }

    /// Takes the worker's list, largest file first.
    pub(crate) fn loaded(&mut self, files: Vec<FileEntry>, truncated: bool) {
        let bytes = files.iter().map(|file| file.bytes).sum();
        self.contents = Contents::Loaded {
            files,
            truncated,
            bytes,
        };
        self.apply_sort();
    }

    /// Drops the excluded path, and everything inside it, from the list.
    pub(crate) fn exclude(&mut self, path: &Path) {
        if let Contents::Loaded { files, bytes, .. } = &mut self.contents {
            let rules = Rules {
                exclusions: vec![path.to_path_buf()],
                ..Rules::default()
            };
            files.retain(|file| !rules.is_excluded(&file.path));
            *bytes = files.iter().map(|file| file.bytes).sum();
        }
    }

    fn sort_by(&mut self, column: SortBy) {
        if self.sort == column {
            self.descending = !self.descending;
        } else {
            self.sort = column;
            // Paths read best A to Z; sizes and dates biggest and newest first.
            self.descending = column != SortBy::Path;
        }
        self.apply_sort();
    }

    fn apply_sort(&mut self) {
        let Contents::Loaded { files, .. } = &mut self.contents else {
            return;
        };
        match (self.sort, self.descending) {
            (SortBy::Path, false) => files.sort_unstable_by(|a, b| a.path.cmp(&b.path)),
            (SortBy::Path, true) => files.sort_unstable_by(|a, b| b.path.cmp(&a.path)),
            (SortBy::Size, false) => files.sort_unstable_by_key(|file| file.bytes),
            (SortBy::Size, true) => files.sort_unstable_by_key(|file| Reverse(file.bytes)),
            (SortBy::Modified, false) => files.sort_unstable_by_key(|file| file.modified),
            (SortBy::Modified, true) => files.sort_unstable_by_key(|file| Reverse(file.modified)),
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the table's header and body read best beside each other"
)]
pub(crate) fn show(
    ctx: &egui::Context,
    texts: &UiText,
    view: &mut FilesView,
) -> Option<FilesAction> {
    let mut open = true;
    let mut action = None;
    let mut sort = None;
    egui::Window::new(&view.title)
        .id(egui::Id::unique("file-preview"))
        .collapsible(false)
        .resizable(true)
        .default_size([960.0, 600.0])
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .open(&mut open)
        .show(ctx, |ui| match &view.contents {
            Contents::Loading => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(texts.files_loading).color(theme::MUTED));
                });
            }
            Contents::Loaded {
                files,
                truncated,
                bytes,
            } => {
                ui.label(texts.files_summary(files.len(), *bytes));
                if *truncated {
                    ui.label(
                        RichText::new(texts.files_truncated(FILE_LIST_LIMIT)).color(theme::MUTED),
                    );
                }
                ui.label(
                    RichText::new(texts.files_hint)
                        .size(theme::FONT_SMALL)
                        .color(theme::MUTED),
                );
                ui.add_space(theme::SPACE_XS);
                if files.is_empty() {
                    ui.label(RichText::new(texts.files_none).color(theme::MUTED));
                    return;
                }
                let arrow = |column: SortBy| {
                    if view.sort != column {
                        ""
                    } else if view.descending {
                        " \u{2193}"
                    } else {
                        " \u{2191}"
                    }
                };
                let row_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::remainder().at_least(240.0).clip(true))
                    .column(Column::initial(90.0).at_least(70.0))
                    .column(Column::initial(140.0).at_least(110.0))
                    .column(Column::auto())
                    .header(row_height, |mut header| {
                        for (column, label) in [
                            (SortBy::Path, texts.files_column_path),
                            (SortBy::Size, texts.files_column_size),
                            (SortBy::Modified, texts.files_column_modified),
                        ] {
                            header.col(|ui| {
                                let text = format!("{label}{}", arrow(column));
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new(text).strong())
                                            .frame(false),
                                    )
                                    .clicked()
                                {
                                    sort = Some(column);
                                }
                            });
                        }
                        header.col(|_| {});
                    })
                    .body(|body| {
                        body.rows(row_height, files.len(), |mut row| {
                            let file = &files[row.index()];
                            row.col(|ui| {
                                ui.add(
                                    egui::Label::new(file.path.display().to_string()).truncate(),
                                )
                                .on_hover_text(file.path.display().to_string());
                            });
                            row.col(|ui| {
                                ui.label(human_bytes(file.bytes));
                            });
                            row.col(|ui| {
                                ui.label(format_modified(file.modified));
                            });
                            row.col(|ui| {
                                if ui
                                    .small_button(texts.files_exclude)
                                    .on_hover_text(texts.files_exclude_hint)
                                    .clicked()
                                {
                                    action = Some(FilesAction::Exclude(file.path.clone()));
                                }
                            });
                        });
                    });
            }
        });
    if let Some(column) = sort {
        view.sort_by(column);
    }
    if !open {
        return Some(FilesAction::Close);
    }
    action
}

/// A modification time in local time, to the minute.
fn format_modified(modified: Option<std::time::SystemTime>) -> String {
    modified
        .and_then(|time| jiff::Timestamp::try_from(time).ok())
        .map(|time| {
            time.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, bytes: u64) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            bytes,
            modified: None,
        }
    }

    #[test]
    fn the_list_sorts_by_each_column_and_drops_excluded_paths() {
        let mut view = FilesView::new("App - Cache".to_owned());
        view.loaded(
            vec![entry("/c/b", 5), entry("/c/a", 50), entry("/c/keep/x", 7)],
            false,
        );
        let Contents::Loaded { files, .. } = &view.contents else {
            panic!("loaded");
        };
        assert_eq!(files[0].bytes, 50, "largest first");

        view.sort_by(SortBy::Path);
        let Contents::Loaded { files, .. } = &view.contents else {
            panic!("loaded");
        };
        assert_eq!(files[0].path, PathBuf::from("/c/a"));

        view.exclude(Path::new("/c/keep"));
        let Contents::Loaded { files, bytes, .. } = &view.contents else {
            panic!("loaded");
        };
        assert_eq!(files.len(), 2);
        assert_eq!(*bytes, 55);
    }

    #[test]
    #[ignore = "timing probe; run with --release --ignored"]
    fn a_million_rows_sort_quickly() {
        let mut view = FilesView::new("Big".to_owned());
        let files = (0..1_000_000u64)
            .map(|index| entry(&format!("/cache/{:07}", (index * 7919) % 1_000_000), index))
            .collect();
        view.loaded(files, false);
        let started = std::time::Instant::now();
        view.sort_by(SortBy::Path);
        view.sort_by(SortBy::Size);
        // Generous, so a slow CI machine passes; the point is "not seconds".
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
    }
}
