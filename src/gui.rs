use gtk4::{
    glib,
    glib::{MainContext, MainLoop},
};

pub mod file_chooser;

pub struct Ui {
    main_loop: MainLoop,
    proxy: UiProxy,
}

fn init() {
    gtk4::init().unwrap();
    glib::set_prgname(Some("xdg-desktop-portal-gtk4"));
}

impl Ui {
    pub fn new() -> Self {
        init();

        let main_loop = MainLoop::new(None, false);
        Self {
            proxy: UiProxy {
                context: main_loop.context().clone(),
            },
            main_loop,
        }
    }

    pub fn run(&self) {
        self.main_loop.run();
    }

    pub fn proxy(&self) -> &UiProxy {
        &self.proxy
    }
}

#[derive(Clone)]
pub struct UiProxy {
    context: MainContext,
}
