use {
    crate::{
        gui::{
            file_chooser,
            file_chooser::{
                ChoiceVariant, FileChooserError, FileChooserUi, Filter, FilterKind, FinalChoice,
            },
            UiProxy,
        },
        portal::{request::run_request, response::Response},
    },
    bstr::{ByteSlice, ByteVec},
    error_reporter::Report,
    serde::Deserializer,
    std::{ffi::CString, path::Path, str::FromStr},
    thiserror::Error,
    url::Url,
    zbus::{
        export::serde::Deserialize,
        interface,
        zvariant::{DeserializeDict, OwnedObjectPath, SerializeDict, Type},
        ObjectServer,
    },
};

pub struct FileChooser {
    proxy: UiProxy,
}

impl FileChooser {
    pub fn new(proxy: &UiProxy) -> Self {
        Self {
            proxy: proxy.clone(),
        }
    }
}

type Choice = (String, String, Vec<(String, String)>, String);

type FileFilter = (String, Vec<(u32, String)>);

#[derive(Type, Debug, Default, PartialEq)]
#[zvariant(signature = "ay")]
struct FilePath(String);

impl<'de> Deserialize<'de> for FilePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <Vec<u8>>::deserialize(deserializer)?;
        let c_string = CString::from_vec_with_nul(bytes)
            .map_err(|_| serde::de::Error::custom("Bytes are not nul-terminated"))?;
        Ok(Self(c_string.into_bytes().into_string_lossy()))
    }
}

#[derive(DeserializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct OpenFileOptions {
    accept_label: Option<String>,
    modal: Option<bool>,
    multiple: Option<bool>,
    directory: Option<bool>,
    filters: Option<Vec<FileFilter>>,
    current_filter: Option<FileFilter>,
    choices: Option<Vec<Choice>>,
    current_folder: Option<FilePath>,
}

#[derive(DeserializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct SaveFileOptions {
    accept_label: Option<String>,
    modal: Option<bool>,
    multiple: Option<bool>,
    filters: Option<Vec<FileFilter>>,
    current_filter: Option<FileFilter>,
    choices: Option<Vec<Choice>>,
    current_name: Option<String>,
    current_folder: Option<FilePath>,
    current_filename: Option<FilePath>,
}

#[derive(DeserializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct SaveFilesOptions {
    accept_label: Option<String>,
    modal: Option<bool>,
    choices: Option<Vec<Choice>>,
    current_folder: Option<FilePath>,
    files: Vec<FilePath>,
}

#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct OpenFileResults {
    uris: Option<Vec<String>>,
    choices: Option<Vec<(String, String)>>,
    current_filter: Option<FileFilter>,
    writable: Option<bool>,
}

#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct SaveFileResults {
    uris: Option<Vec<String>>,
    choices: Option<Vec<(String, String)>>,
    current_filter: Option<FileFilter>,
}

#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct SaveFilesResults {
    uris: Option<Vec<String>>,
    choices: Option<Vec<(String, String)>>,
}

#[derive(Debug, Error)]
enum SaveFilesError {
    #[error("User did not select exactly one path")]
    NotExactlyOnePath,
    #[error("Client tried to save an absolute path")]
    AbsolutePath,
    #[error("Client tried to save a path with multiple components")]
    MultipleComponents,
    #[error("Client tried to save `.` or `..`")]
    SpecialPath,
    #[error("The selected path is not a valid URI")]
    SelectedNotValidUrl(#[source] url::ParseError),
    #[error("The selected path is not a path")]
    SelectedNotValidPath,
    #[error("The computed unique path is not a valid URI")]
    UniqueNotValidUrl,
    #[error(transparent)]
    Ui(FileChooserError),
}

impl FileChooser {
    async fn open_file_impl(
        &self,
        app_id: String,
        parent_window: String,
        title: String,
        options: OpenFileOptions,
    ) -> Response<OpenFileResults> {
        let res = FileChooserUi {
            title,
            multiple: options.multiple.unwrap_or(false),
            accept_label: options.accept_label,
            modal: options.modal.unwrap_or(true),
            directory: options.directory.unwrap_or(false),
            filters: options.filters.map(map_filters),
            current_filter: options.current_filter.map(map_filter),
            current_name: None,
            current_folder: options.current_folder.map(map_cstr),
            current_filename: None,
            choices: options.choices.map(map_choices),
            save: false,
            parent_window,
            app_id,
        }
        .run(&self.proxy)
        .await;
        match res {
            Ok(res) => Response::success(OpenFileResults {
                uris: Some(res.uris),
                choices: res.final_choices.map(map_final_choices),
                current_filter: res.current_filter.map(unmap_filter),
                writable: Some(res.writeable),
            }),
            Err(e) => {
                log::error!("OpenFile failed: {}", Report::new(e));
                Response::cancelled()
            }
        }
    }

    async fn save_file_impl(
        &self,
        app_id: String,
        parent_window: String,
        title: String,
        options: SaveFileOptions,
    ) -> Response<SaveFileResults> {
        let res = FileChooserUi {
            title,
            multiple: options.multiple.unwrap_or(false),
            accept_label: options.accept_label,
            modal: options.modal.unwrap_or(true),
            directory: false,
            filters: options.filters.map(map_filters),
            current_filter: options.current_filter.map(map_filter),
            current_name: options.current_name,
            current_folder: options.current_folder.map(map_cstr),
            current_filename: options.current_filename.map(map_cstr),
            choices: options.choices.map(map_choices),
            save: true,
            parent_window,
            app_id,
        }
        .run(&self.proxy)
        .await;
        match res {
            Ok(res) => Response::success(SaveFileResults {
                uris: Some(res.uris),
                choices: res.final_choices.map(map_final_choices),
                current_filter: res.current_filter.map(unmap_filter),
            }),
            Err(e) => {
                log::error!("SaveFile failed: {}", Report::new(e));
                Response::cancelled()
            }
        }
    }

    async fn try_save_files_impl(
        &self,
        app_id: String,
        parent_window: String,
        title: String,
        options: SaveFilesOptions,
    ) -> Result<SaveFilesResults, SaveFilesError> {
        for file in &options.files {
            let file = Path::new(&file.0);
            // none of the following can be used securely with the current UI
            if file.is_absolute() {
                return Err(SaveFilesError::AbsolutePath);
            }
            if file.components().count() > 1 {
                return Err(SaveFilesError::MultipleComponents);
            }
            if file == Path::new(".") || file == Path::new("..") {
                return Err(SaveFilesError::SpecialPath);
            }
        }
        let mut res = FileChooserUi {
            title,
            multiple: false,
            accept_label: options.accept_label,
            modal: options.modal.unwrap_or(true),
            directory: true,
            filters: None,
            current_filter: None,
            current_name: None,
            current_folder: options.current_folder.map(map_cstr),
            current_filename: None,
            choices: options.choices.map(map_choices),
            save: true,
            parent_window,
            app_id,
        }
        .run(&self.proxy)
        .await
        .map_err(SaveFilesError::Ui)?;
        if res.uris.len() != 1 {
            return Err(SaveFilesError::NotExactlyOnePath);
        }
        let base = Url::from_str(&res.uris.pop().unwrap())
            .map_err(SaveFilesError::SelectedNotValidUrl)?
            .to_file_path()
            .map_err(|_| SaveFilesError::SelectedNotValidPath)?;
        let mut uris = vec![];
        for file in &options.files {
            let mut path = base.join(&file.0);
            if path.exists() {
                let (prefix, dot, suffix) = match file.0.split_once('.') {
                    Some((prefix, suffix)) => (prefix, ".", suffix),
                    _ => (file.0.as_str(), "", ""),
                };
                for i in 1u64.. {
                    path = base.join(format!("{prefix} ({i}){dot}{suffix}"));
                    if !path.exists() {
                        break;
                    }
                }
            }
            uris.push(
                Url::from_file_path(&path)
                    .map_err(|_| SaveFilesError::UniqueNotValidUrl)?
                    .to_string(),
            );
        }
        Ok(SaveFilesResults {
            uris: Some(uris),
            choices: res.final_choices.map(map_final_choices),
        })
    }

    async fn save_files_impl(
        &self,
        app_id: String,
        parent_window: String,
        title: String,
        options: SaveFilesOptions,
    ) -> Response<SaveFilesResults> {
        match self
            .try_save_files_impl(app_id, parent_window, title, options)
            .await
        {
            Ok(res) => Response::success(res),
            Err(e) => {
                log::error!("SaveFiles failed: {}", Report::new(e));
                Response::cancelled()
            }
        }
    }
}

#[interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: OpenFileOptions,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Response<OpenFileResults> {
        run_request(
            server,
            handle,
            self.open_file_impl(app_id, parent_window, title, options),
        )
        .await
    }

    async fn save_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: SaveFileOptions,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Response<SaveFileResults> {
        run_request(
            server,
            handle,
            self.save_file_impl(app_id, parent_window, title, options),
        )
        .await
    }

    async fn save_files(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: SaveFilesOptions,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Response<SaveFilesResults> {
        run_request(
            server,
            handle,
            self.save_files_impl(app_id, parent_window, title, options),
        )
        .await
    }
}

fn map_filters(f: Vec<FileFilter>) -> Vec<Filter> {
    f.into_iter().map(map_filter).collect()
}

fn map_filter(f: FileFilter) -> Filter {
    Filter {
        name: f.0,
        elements: f
            .1
            .into_iter()
            .flat_map(|(kind, value)| match kind {
                0 => Some(FilterKind::Glob(value)),
                1 => Some(FilterKind::Mime(value)),
                _ => None,
            })
            .collect(),
    }
}

fn unmap_filter(f: Filter) -> FileFilter {
    (
        f.name,
        f.elements
            .into_iter()
            .map(|f| match f {
                FilterKind::Glob(v) => (0, v),
                FilterKind::Mime(v) => (1, v),
            })
            .collect(),
    )
}

fn map_cstr(f: FilePath) -> String {
    f.0.as_bytes().to_str_lossy().into_owned()
}

fn map_choices(c: Vec<Choice>) -> Vec<file_chooser::Choice> {
    c.into_iter().map(map_choice).collect()
}

fn map_choice(c: Choice) -> file_chooser::Choice {
    file_chooser::Choice {
        id: c.0,
        label: c.1,
        default: c.3,
        variants: c
            .2
            .into_iter()
            .map(|c| ChoiceVariant {
                id: c.0,
                label: c.1,
            })
            .collect(),
    }
}

fn map_final_choices(c: Vec<FinalChoice>) -> Vec<(String, String)> {
    c.into_iter().map(map_final_choice).collect()
}

fn map_final_choice(c: FinalChoice) -> (String, String) {
    (c.id, c.variant_id)
}
