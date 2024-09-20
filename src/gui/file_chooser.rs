use {
    crate::{gui::UiProxy, utils::file_chooser_ext::FileChooserExtManualFixed},
    async_channel::{Receiver, Sender},
    gdk4_wayland::WaylandToplevel,
    gtk4::{
        gio::File,
        glib::MainContext,
        prelude::{
            Cast, DialogExt, FileChooserExt, FileChooserExtManual, FileExt, GtkWindowExt,
            NativeExt, RecentManagerExt, WidgetExt,
        },
        FileChooserAction, FileChooserDialog, FileFilter, RecentData, RecentManager, ResponseType,
        Widget, Window,
    },
    rust_i18n::t,
    std::{
        cell::Cell,
        collections::{HashMap, HashSet},
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum FileChooserError {
    #[error("Operation could not be started")]
    Closed,
    #[error("Operation was rejected")]
    Rejected,
}

#[derive(Eq, PartialEq, Clone)]
pub struct Filter {
    pub name: String,
    pub elements: Vec<FilterKind>,
}

#[derive(Eq, PartialEq, Clone)]
pub enum FilterKind {
    Glob(String),
    Mime(String),
}

pub struct Choice {
    pub id: String,
    pub label: String,
    pub default: String,
    pub variants: Vec<ChoiceVariant>,
}

pub struct ChoiceVariant {
    pub id: String,
    pub label: String,
}

pub struct FinalChoice {
    pub id: String,
    pub variant_id: String,
}

pub struct FileChooserUi {
    pub title: String,
    pub multiple: bool,
    pub accept_label: Option<String>,
    pub modal: bool,
    pub directory: bool,
    pub filters: Option<Vec<Filter>>,
    pub current_filter: Option<Filter>,
    pub current_name: Option<String>,
    pub current_folder: Option<String>,
    pub current_filename: Option<String>,
    pub choices: Option<Vec<Choice>>,
    pub save: bool,
    pub parent_window: String,
    pub app_id: String,
}

pub struct FileChooserResult {
    pub uris: Vec<String>,
    pub current_filter: Option<Filter>,
    pub final_choices: Option<Vec<FinalChoice>>,
    pub writeable: bool,
}

struct DialogData {
    dialog: FileChooserDialog,
    read_only_choice: String,
    filters: HashMap<FileFilter, Filter>,
}

impl FileChooserUi {
    pub async fn run(self, proxy: &UiProxy) -> Result<FileChooserResult, FileChooserError> {
        let (send, recv) = async_channel::bounded(1);
        let (_send, close_on_close) = async_channel::bounded(1);
        let context = proxy.context.clone();
        proxy
            .context
            .invoke(move || self.run_impl(send, context, close_on_close));
        recv.recv().await.map_err(|_| FileChooserError::Closed)?
    }

    fn run_impl(
        self,
        send: Sender<Result<FileChooserResult, FileChooserError>>,
        context: MainContext,
        close_on_close: Receiver<()>,
    ) {
        let DialogData {
            dialog,
            read_only_choice,
            filters,
        } = self.build_dialog();
        let current_filter = Rc::new(Cell::new(dialog.filter()));
        let cf = current_filter.clone();
        dialog.connect_filter_notify(move |f| cf.set(f.filter()));
        let cf = current_filter.clone();
        dialog.connect_response(move |dialog, r| {
            let res = match r {
                ResponseType::Ok => {
                    let files: Vec<_> = dialog
                        .files()
                        .into_iter()
                        .map(|f| f.unwrap().downcast::<File>().unwrap().uri().into())
                        .collect();
                    add_recent(&self.app_id, &files);
                    let filter = cf.take().and_then(|f| filters.get(&f).cloned());
                    let choices: Vec<_> = self
                        .choices
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .flat_map(|c| {
                            dialog.choice(&c.id).map(|v| FinalChoice {
                                id: c.id.to_string(),
                                variant_id: v.to_string(),
                            })
                        })
                        .collect();
                    let writeable = dialog
                        .choice(&read_only_choice)
                        .map(|v| v == "false")
                        .unwrap_or(false);
                    Ok(FileChooserResult {
                        uris: files,
                        current_filter: filter,
                        final_choices: self.choices.is_some().then_some(choices),
                        writeable,
                    })
                }
                _ => Err(FileChooserError::Rejected),
            };
            let _ = send.send_blocking(res);
            dialog.close();
        });
        dialog.show();
        context.spawn_local(async move {
            let _ = close_on_close.recv().await;
            dialog.close();
        });
    }

    fn build_dialog(&self) -> DialogData {
        let action = match (self.directory, self.save) {
            (true, _) => FileChooserAction::SelectFolder,
            (_, true) => FileChooserAction::Save,
            (false, _) => FileChooserAction::Open,
        };
        let accept_label = match self.save {
            true => t!("_Save"),
            false => t!("_Open"),
        };
        let buttons = [
            (
                self.accept_label.as_deref().unwrap_or(&accept_label),
                ResponseType::Ok,
            ),
            (&t!("_Cancel"), ResponseType::Cancel),
        ];
        let dialog =
            FileChooserDialog::new(Some(self.title.clone()), Window::NONE, action, &buttons);
        dialog.set_select_multiple(self.multiple);
        dialog.set_modal(self.modal);
        dialog.set_default_response(ResponseType::Ok);
        let mut filters_map = HashMap::new();
        if let Some(f) = &self.filters {
            for filter in f {
                let is_current = self.current_filter.as_ref() == Some(filter);
                let f = map_filter(filter);
                dialog.add_filter(&f);
                if is_current {
                    dialog.set_filter(&f);
                }
                filters_map.insert(f, filter.clone());
            }
        }
        if let Some(f) = &self.current_name {
            dialog.set_current_name(f);
        }
        if let Some(f) = &self.current_folder {
            let _ = dialog.set_current_folder(Some(&File::for_path(f)));
        }
        if let Some(f) = &self.current_filename {
            let _ = dialog.set_file(&File::for_uri(f));
        }
        let mut read_only_id = String::new();
        if action == FileChooserAction::Open {
            let choice_ids: HashSet<_> = self
                .choices
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|c| c.id.as_str())
                .collect();
            read_only_id = "_read_only".to_string();
            while choice_ids.contains(read_only_id.as_str()) {
                read_only_id.push('_');
            }
            dialog.add_choice_fixed(&read_only_id, t!("Open files read-only").as_ref(), &[]);
            dialog.set_choice(&read_only_id, "true");
        }
        if let Some(choices) = &self.choices {
            for choice in choices {
                let mut variants = vec![];
                for variant in &choice.variants {
                    variants.push((variant.id.as_str(), variant.label.as_str()));
                }
                dialog.add_choice_fixed(&choice.id, &choice.label, &variants);
                dialog.set_choice(&choice.id, &choice.default);
            }
        }
        dialog.upcast_ref::<Widget>().realize();
        if let Some(parent) = self.parent_window.strip_prefix("wayland:") {
            if let Some(surface) = dialog.surface() {
                if let Some(toplevel) = surface.downcast_ref::<WaylandToplevel>() {
                    toplevel.set_transient_for_exported(parent);
                }
            }
        }
        DialogData {
            dialog,
            read_only_choice: read_only_id,
            filters: filters_map,
        }
    }
}

fn map_filter(f: &Filter) -> FileFilter {
    let gf = FileFilter::new();
    gf.set_name(Some(&f.name));
    for kind in &f.elements {
        match kind {
            FilterKind::Glob(g) => gf.add_pattern(g),
            FilterKind::Mime(m) => gf.add_mime_type(m),
        }
    }
    gf
}

fn add_recent(app_id: &str, uris: &[String]) {
    let manager = RecentManager::default();
    for uri in uris {
        manager.add_full(
            uri,
            &RecentData::new(
                None,
                None,
                "application/octet-stream",
                app_id,
                "false",
                &[],
                false,
            ),
        );
    }
}
