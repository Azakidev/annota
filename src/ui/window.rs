use sourceview5::prelude::*;

use ashpd::desktop::file_chooser::{Choice, FileFilter, SelectedFiles};
use sourceview5::traits::BufferExt;
use string_join::Join;
use url::Url;

use adw::subclass::prelude::*;
use gtk::{gio, glib};
use gtk::{prelude::*, template_callbacks};

use crate::application::ExampleApplication;
use crate::config::{APP_ID, PROFILE};

mod imp {
    use gtk::glib::Propagation;

    use super::*;

    #[derive(Debug, gtk::CompositeTemplate)]
    #[template(resource = "/com/Azakidev/Annota/ui/window.ui")]
    pub struct ExampleApplicationWindow {
        pub settings: gio::Settings,
        #[template_child]
        pub headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub overview: TemplateChild<adw::TabOverview>,
        #[template_child]
        pub tab_view: TemplateChild<adw::TabView>,
        #[template_child]
        pub overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
    }

    impl Default for ExampleApplicationWindow {
        fn default() -> Self {
            Self {
                headerbar: TemplateChild::default(),
                settings: gio::Settings::new(APP_ID),
                overview: TemplateChild::default(),
                tab_view: TemplateChild::default(),
                overlay: TemplateChild::default(),
                stack: TemplateChild::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExampleApplicationWindow {
        const NAME: &'static str = "ExampleApplicationWindow";
        type Type = super::ExampleApplicationWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();

            klass.install_action("win.new-tab", None, |win, _, _| {
                construct_tab(win, "New tab", "Empty editor");
            });
            klass.install_action_async("win.open-files", None, |win, _, _| async move {
                win.open_file().await;
            });

            klass.install_action("win.close-tab", None, |win, _, _| {
                let stack = &win.imp().stack;
                let tab_view = &win.imp().tab_view;
                let page = tab_view.selected_page();

                if stack.visible_child_name().unwrap().as_str() != "empty" {
                    if tab_view.n_pages() != 0 {
                        tab_view.close_page(page.unwrap().as_ref());
                    }

                    if tab_view.n_pages() == 0 {
                        stack.set_visible_child_name("empty")
                    }
                }
            });
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ExampleApplicationWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            // Load latest window state
            obj.load_window_size();
        }
    }

    impl WidgetImpl for ExampleApplicationWindow {}
    impl WindowImpl for ExampleApplicationWindow {
        // Save window state on delete event
        fn close_request(&self) -> Propagation {
            if let Err(err) = self.obj().save_window_size() {
                log::warn!("Failed to save window state, {}", &err);
            }

            // Pass close request on to the parent
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for ExampleApplicationWindow {}
    impl AdwApplicationWindowImpl for ExampleApplicationWindow {}
}

glib::wrapper! {
    pub struct ExampleApplicationWindow(ObjectSubclass<imp::ExampleApplicationWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

#[template_callbacks]
impl ExampleApplicationWindow {
    pub fn new(app: &ExampleApplication) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    #[template_callback]
    pub fn create_tab(tab_overview: &adw::TabOverview) -> adw::TabPage {
        let tab_view = tab_overview.view().unwrap();

        let page = adw::StatusPage::builder()
            .title("Empty editor here.")
            .vexpand(true)
            .build();
        let tab_page = tab_view.append(&page);
        tab_page.set_title("New tab");
        tab_page.set_live_thumbnail(true);
        tab_page
    }

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let imp = self.imp();

        let (width, height) = self.default_size();

        imp.settings.set_int("window-width", width)?;
        imp.settings.set_int("window-height", height)?;

        imp.settings
            .set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let imp = self.imp();

        let width = imp.settings.int("window-width");
        let height = imp.settings.int("window-height");
        let is_maximized = imp.settings.boolean("is-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }

    async fn open_file(&self) {
        let imp = self.imp();
        let overlay = &imp.overlay;

        let request = SelectedFiles::open_file()
            .title("Open a file")
            .accept_label("Open")
            .modal(true)
            .multiple(true)
            .directory(false)
            .choice(Choice::new("encoding", "Encoding", "utf8").insert("utf8", "Unicode (UTF-8)"))
            .filter(FileFilter::new("Plain text").mimetype("text/plain"))
            .filter(FileFilter::new("All files").mimetype("application/octet-stream"));

        match request.send().await.and_then(|r| r.response()) {
            Ok(files) => {
                for uri in files.uris() {
                    let toast = adw::Toast::builder()
                        .title(
                            " ".join([
                                "Opened:",
                                Url::parse(uri.as_str())
                                    .unwrap()
                                    .path_segments()
                                    .unwrap()
                                    .last()
                                    .unwrap(),
                            ]),
                        )
                        .timeout(1)
                        .build();
                    overlay.add_toast(toast);

                    construct_editor(
                        self,
                        Url::parse(uri.as_str())
                            .unwrap()
                            .path_segments()
                            .unwrap()
                            .last()
                            .unwrap(),
                        uri.clone(),
                    )
                }
            }
            Err(err) => {
                if err.to_string() != *"Portal request didn't succeed: Cancelled" {
                    log::warn!("Failed to load file: {err}");
                    let errtoast = adw::Toast::builder()
                        .title("There was an error loading the file!")
                        .timeout(1)
                        .build();
                    overlay.add_toast(errtoast);
                }
            }
        }
    }
}

pub fn construct_tab(win: &ExampleApplicationWindow, tab_title: &str, title: &str) {
    let tab_view = &win.imp().tab_view;
    let stack = &win.imp().stack;

    let page = adw::StatusPage::builder()
        .title(" ".join([title, " here."]))
        .vexpand(true)
        .build();

    let tab_page = tab_view.append(&page);
    tab_page.set_title(tab_title);
    tab_page.set_live_thumbnail(true);
    tab_view.set_selected_page(&tab_page);

    if stack.visible_child_name().unwrap().as_str() == "empty" {
        stack.set_visible_child_name("main")
    }
}

pub fn construct_editor(win: &ExampleApplicationWindow, tab_title: &str, uri: Url) {
    let tab_view = &win.imp().tab_view;
    let stack = &win.imp().stack;
    let file = std::path::Path::new(uri.as_str());

    let buffer = sourceview5::Buffer::new(None);
    buffer.set_highlight_syntax(true);
    if let Some(ref language) = sourceview5::LanguageManager::new().guess_language(Some(file), None)
    {
        buffer.set_language(Some(language));
    }
    if let Some(ref scheme) = sourceview5::StyleSchemeManager::new().scheme("classic-dark") {
        buffer.set_style_scheme(Some(scheme));
    }

    let file = gio::File::for_path(uri.path());
    let file = sourceview5::File::builder().location(&file).build();
    let loader = sourceview5::FileLoader::new(&buffer, &file);
    loader.load_async(glib::Priority::default(), gio::Cancellable::NONE, |res| {
        println!("loaded: {:?}", res);
    });

    let view = sourceview5::View::with_buffer(&buffer);
    view.set_monospace(true);
    view.set_background_pattern(sourceview5::BackgroundPatternType::None);
    view.set_show_line_numbers(true);
    view.set_highlight_current_line(true);
    view.set_tab_width(4);
    view.set_hexpand(true);
    view.set_vexpand(true);
    view.set_wrap_mode(gtk::WrapMode::Word);

    let scrollable = gtk::ScrolledWindow::builder().child(&view).build();
    scrollable.set_vexpand(true);

    let tab_page = tab_view.append(&scrollable);
    tab_page.set_title(tab_title);
    tab_page.set_live_thumbnail(true);
    tab_view.set_selected_page(&tab_page);

    if stack.visible_child_name().unwrap().as_str() == "empty" {
        stack.set_visible_child_name("main")
    }
}
