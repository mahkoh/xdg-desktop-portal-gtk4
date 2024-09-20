use {
    gtk4::{
        ffi,
        glib::{
            translate::{mut_override, ToGlibPtr},
            IntoGStr,
        },
        prelude::IsA,
        FileChooser,
    },
    std::ptr,
};

// https://github.com/gtk-rs/gtk4-rs/pull/1834
pub trait FileChooserExtManualFixed: IsA<FileChooser> + 'static {
    fn add_choice_fixed(&self, id: impl IntoGStr, label: impl IntoGStr, options: &[(&str, &str)]) {
        let stashes_ids = options
            .iter()
            .map(|o| o.0.to_glib_none())
            .collect::<Vec<_>>();
        let stashes_labels = options
            .iter()
            .map(|o| o.1.to_glib_none())
            .collect::<Vec<_>>();
        let stashes_id_ptrs = stashes_ids
            .iter()
            .map(|o| o.0)
            .chain(Some(ptr::null()))
            .collect::<Vec<*const libc::c_char>>();
        let stashes_label_ptrs = stashes_labels
            .iter()
            .map(|o| o.0)
            .chain(Some(ptr::null()))
            .collect::<Vec<*const libc::c_char>>();

        unsafe {
            let (options_ids, options_labels) = if options.is_empty() {
                (ptr::null(), ptr::null())
            } else {
                (stashes_id_ptrs.as_ptr(), stashes_label_ptrs.as_ptr())
            };

            id.run_with_gstr(|id| {
                label.run_with_gstr(|label| {
                    ffi::gtk_file_chooser_add_choice(
                        self.as_ref().to_glib_none().0,
                        id.as_ptr(),
                        label.as_ptr(),
                        mut_override(options_ids),
                        mut_override(options_labels),
                    );
                });
            });
        }
    }
}

impl<O: IsA<FileChooser>> FileChooserExtManualFixed for O {}
