mod appsettings;
mod appwindowactions;
pub mod imexport;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::{prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use gtk4::{
    gdk, gio, glib, glib::clone, Align, Application, ArrowType, Box, Button,
    CompositeTemplate, CornerType, CssProvider, EventControllerScroll,
    EventControllerScrollFlags, EventSequenceState, FileChooserNative,
    GestureDrag, GestureZoom, Grid, IconTheme, Inhibit, PackType, PolicyType,
    PositionType, ProgressBar, PropagationPhase, Revealer, ScrolledWindow,
    Separator, StyleContext, ToggleButton,
};
use once_cell::sync::Lazy;

use crate::{
    app::RnoteApp,
    canvas::RnoteCanvas,
    config,
    penssidebar::PensSideBar,
    settingspanel::SettingsPanel,
    workspacebrowser::WorkspaceBrowser,
    {dialogs, mainheader::MainHeader},
};
use rnote_engine::{
    engine::EngineTask, pens::penholder::PenStyle, Camera, WidgetFlags,
};

mod imp {
    use super::*;

    #[allow(missing_debug_implementations)]
    #[derive(CompositeTemplate)]
    #[template(resource = "/com/github/flxzt/rnote/ui/appwindow.ui")]
    pub struct RnoteAppWindow {
        pub app_settings: gio::Settings,
        pub filechoosernative: Rc<RefCell<Option<FileChooserNative>>>,
        pub autosave_source_id: Rc<RefCell<Option<glib::SourceId>>>,
        pub progresspulse_source_id: Rc<RefCell<Option<glib::SourceId>>>,

        pub unsaved_changes: Cell<bool>,
        pub autosave: Cell<bool>,
        pub autosave_interval_secs: Cell<u32>,
        pub righthanded: Cell<bool>,
        pub permanently_hide_canvas_scrollbars: Cell<bool>,

        pub canvas_touch_drag_gesture: GestureDrag,
        pub canvas_drag_empty_area_gesture: GestureDrag,
        pub canvas_zoom_gesture: GestureZoom,
        pub canvas_zoom_scroll_controller: EventControllerScroll,
        pub canvas_mouse_drag_middle_gesture: GestureDrag,

        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub main_grid: TemplateChild<Grid>,
        #[template_child]
        pub canvas_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub canvas_quickactions_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub canvas_fixedsize_quickactions_revealer: TemplateChild<Revealer>,
        #[template_child]
        pub undo_button: TemplateChild<Button>,
        #[template_child]
        pub redo_button: TemplateChild<Button>,
        #[template_child]
        pub canvas_scroller: TemplateChild<ScrolledWindow>,
        #[template_child]
        pub canvas_progressbar: TemplateChild<ProgressBar>,
        #[template_child]
        pub canvas: TemplateChild<RnoteCanvas>,
        #[template_child]
        pub settings_panel: TemplateChild<SettingsPanel>,
        #[template_child]
        pub sidebar_scroller: TemplateChild<ScrolledWindow>,
        #[template_child]
        pub sidebar_grid: TemplateChild<Grid>,
        #[template_child]
        pub sidebar_sep: TemplateChild<Separator>,
        #[template_child]
        pub flap: TemplateChild<adw::Flap>,
        #[template_child]
        pub flap_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub flap_header: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub flap_resizer: TemplateChild<gtk4::Box>,
        #[template_child]
        pub flap_resizer_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub flap_close_button: TemplateChild<Button>,
        #[template_child]
        pub flap_stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub workspacebrowser: TemplateChild<WorkspaceBrowser>,
        #[template_child]
        pub flapreveal_toggle: TemplateChild<ToggleButton>,
        #[template_child]
        pub flap_menus_box: TemplateChild<Box>,
        #[template_child]
        pub mainheader: TemplateChild<MainHeader>,
        #[template_child]
        pub narrow_pens_toggles_revealer: TemplateChild<Revealer>,
        #[template_child]
        pub narrow_brush_toggle: TemplateChild<ToggleButton>,
        #[template_child]
        pub narrow_shaper_toggle: TemplateChild<ToggleButton>,
        #[template_child]
        pub narrow_eraser_toggle: TemplateChild<ToggleButton>,
        #[template_child]
        pub narrow_selector_toggle: TemplateChild<ToggleButton>,
        #[template_child]
        pub narrow_typewriter_toggle: TemplateChild<ToggleButton>,
        #[template_child]
        pub narrow_tools_toggle: TemplateChild<ToggleButton>,
        #[template_child]
        pub penssidebar: TemplateChild<PensSideBar>,
    }

    impl Default for RnoteAppWindow {
        fn default() -> Self {
            let canvas_touch_drag_gesture = GestureDrag::builder()
                .name("canvas_touch_drag_gesture")
                .touch_only(true)
                .propagation_phase(PropagationPhase::Bubble)
                .build();

            let canvas_drag_empty_area_gesture = GestureDrag::builder()
                .name("canvas_mouse_drag_empty_area_gesture")
                .button(gdk::BUTTON_PRIMARY)
                .propagation_phase(PropagationPhase::Bubble)
                .build();

            let canvas_zoom_gesture = GestureZoom::builder()
                .name("canvas_zoom_gesture")
                .propagation_phase(PropagationPhase::Capture)
                .build();

            let canvas_zoom_scroll_controller =
                EventControllerScroll::builder()
                    .name("canvas_zoom_scroll_controller")
                    .propagation_phase(PropagationPhase::Bubble)
                    .flags(EventControllerScrollFlags::VERTICAL)
                    .build();

            let canvas_mouse_drag_middle_gesture = GestureDrag::builder()
                .name("canvas_mouse_drag_middle_gesture")
                .button(gdk::BUTTON_MIDDLE)
                .propagation_phase(PropagationPhase::Bubble)
                .build();

            Self {
                app_settings: gio::Settings::new(config::APP_ID),
                filechoosernative: Rc::new(RefCell::new(None)),
                autosave_source_id: Rc::new(RefCell::new(None)),
                progresspulse_source_id: Rc::new(RefCell::new(None)),

                unsaved_changes: Cell::new(false),
                autosave: Cell::new(true),
                autosave_interval_secs: Cell::new(
                    super::RnoteAppWindow::AUTOSAVE_INTERVAL_DEFAULT,
                ),
                righthanded: Cell::new(true),
                permanently_hide_canvas_scrollbars: Cell::new(false),

                canvas_touch_drag_gesture,
                canvas_drag_empty_area_gesture,
                canvas_zoom_gesture,
                canvas_zoom_scroll_controller,
                canvas_mouse_drag_middle_gesture,

                toast_overlay: TemplateChild::<adw::ToastOverlay>::default(),
                main_grid: TemplateChild::<Grid>::default(),
                canvas_box: TemplateChild::<gtk4::Box>::default(),
                canvas_quickactions_box: TemplateChild::<gtk4::Box>::default(),
                canvas_fixedsize_quickactions_revealer:
                    TemplateChild::<Revealer>::default(),
                undo_button: TemplateChild::<Button>::default(),
                redo_button: TemplateChild::<Button>::default(),
                canvas_progressbar: TemplateChild::<ProgressBar>::default(),
                canvas_scroller: TemplateChild::<ScrolledWindow>::default(),
                canvas: TemplateChild::<RnoteCanvas>::default(),
                settings_panel: TemplateChild::<SettingsPanel>::default(),
                sidebar_scroller: TemplateChild::<ScrolledWindow>::default(),
                sidebar_grid: TemplateChild::<Grid>::default(),
                sidebar_sep: TemplateChild::<Separator>::default(),
                flap: TemplateChild::<adw::Flap>::default(),
                flap_box: TemplateChild::<gtk4::Box>::default(),
                flap_header: TemplateChild::<adw::HeaderBar>::default(),
                flap_resizer: TemplateChild::<gtk4::Box>::default(),
                flap_resizer_box: TemplateChild::<gtk4::Box>::default(),
                flap_close_button: TemplateChild::<Button>::default(),
                flap_stack: TemplateChild::<adw::ViewStack>::default(),
                workspacebrowser: TemplateChild::<WorkspaceBrowser>::default(),
                flapreveal_toggle: TemplateChild::<ToggleButton>::default(),
                flap_menus_box: TemplateChild::<Box>::default(),
                mainheader: TemplateChild::<MainHeader>::default(),
                narrow_pens_toggles_revealer:
                    TemplateChild::<Revealer>::default(),
                narrow_brush_toggle: TemplateChild::<ToggleButton>::default(),
                narrow_shaper_toggle: TemplateChild::<ToggleButton>::default(),
                narrow_typewriter_toggle:
                    TemplateChild::<ToggleButton>::default(),
                narrow_eraser_toggle: TemplateChild::<ToggleButton>::default(),
                narrow_selector_toggle: TemplateChild::<ToggleButton>::default(
                ),
                narrow_tools_toggle: TemplateChild::<ToggleButton>::default(),
                penssidebar: TemplateChild::<PensSideBar>::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RnoteAppWindow {
        const NAME: &'static str = "RnoteAppWindow";
        type Type = super::RnoteAppWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RnoteAppWindow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let _windowsettings = obj.settings();

            // Add input controllers
            obj.canvas_scroller()
                .add_controller(&self.canvas_touch_drag_gesture);
            obj.canvas_scroller()
                .add_controller(&self.canvas_drag_empty_area_gesture);
            obj.canvas_scroller()
                .add_controller(&self.canvas_zoom_gesture);
            obj.canvas_scroller()
                .add_controller(&self.canvas_zoom_scroll_controller);
            obj.canvas_scroller()
                .add_controller(&self.canvas_mouse_drag_middle_gesture);

            if config::PROFILE == "devel" {
                obj.add_css_class("devel");
            }

            // Load the application css
            let css = CssProvider::new();
            css.load_from_resource(
                (String::from(config::APP_IDPATH) + "ui/style.css").as_str(),
            );

            let display = gdk::Display::default().unwrap();
            StyleContext::add_provider_for_display(
                &display,
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            self.setup_flap(obj);

            // pens narrow toggles
            self.narrow_brush_toggle.connect_toggled(clone!(@weak obj as appwindow => move |narrow_brush_toggle| {
                if narrow_brush_toggle.is_active() {
                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "pen-style", Some(&PenStyle::Brush.nick().to_variant()));
                }
            }));

            self.narrow_shaper_toggle.connect_toggled(clone!(@weak obj as appwindow => move |narrow_shaper_toggle| {
                if narrow_shaper_toggle.is_active() {
                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "pen-style", Some(&PenStyle::Shaper.nick().to_variant()));
                }
            }));

            self.narrow_typewriter_toggle.connect_toggled(clone!(@weak obj as appwindow => move |narrow_typewriter_toggle| {
                if narrow_typewriter_toggle.is_active() {
                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "pen-style", Some(&PenStyle::Typewriter.nick().to_variant()));
                }
            }));

            self.narrow_eraser_toggle.connect_toggled(clone!(@weak obj as appwindow => move |narrow_eraser_toggle| {
                if narrow_eraser_toggle.is_active() {
                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "pen-style", Some(&PenStyle::Eraser.nick().to_variant()));
                }
            }));

            self.narrow_selector_toggle.connect_toggled(clone!(@weak obj as appwindow => move |narrow_selector_toggle| {
                if narrow_selector_toggle.is_active() {
                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "pen-style", Some(&PenStyle::Selector.nick().to_variant()));
                }
            }));

            self.narrow_tools_toggle.connect_toggled(clone!(@weak obj as appwindow => move |narrow_tools_toggle| {
                if narrow_tools_toggle.is_active() {
                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "pen-style", Some(&PenStyle::Tools.nick().to_variant()));
                }
            }));
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoolean::new(
                        "unsaved-changes",
                        "unsaved-changes",
                        "unsaved-changes",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    // autosave
                    glib::ParamSpecBoolean::new(
                        "autosave",
                        "autosave",
                        "autosave",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    // autosave interval in secs
                    glib::ParamSpecUInt::new(
                        "autosave-interval-secs",
                        "autosave-interval-secs",
                        "autosave-interval-secs",
                        5,
                        u32::MAX,
                        super::RnoteAppWindow::AUTOSAVE_INTERVAL_DEFAULT,
                        glib::ParamFlags::READWRITE,
                    ),
                    // righthanded
                    glib::ParamSpecBoolean::new(
                        "righthanded",
                        "righthanded",
                        "righthanded",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    // permanently hide canvas scrollbars
                    glib::ParamSpecBoolean::new(
                        "permanently-hide-canvas-scrollbars",
                        "permanently-hide-canvas-scrollbars",
                        "permanently-hide-canvas-scrollbars",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            pspec: &glib::ParamSpec,
        ) -> glib::Value {
            match pspec.name() {
                "unsaved-changes" => self.unsaved_changes.get().to_value(),
                "autosave" => self.autosave.get().to_value(),
                "autosave-interval-secs" => {
                    self.autosave_interval_secs.get().to_value()
                },
                "righthanded" => self.righthanded.get().to_value(),
                "permanently-hide-canvas-scrollbars" => {
                    self.permanently_hide_canvas_scrollbars.get().to_value()
                },
                _ => unimplemented!(),
            }
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "unsaved-changes" => {
                    let unsaved_changes: bool = value
                        .get()
                        .expect("The value needs to be of type `bool`.");
                    self.unsaved_changes.replace(unsaved_changes);
                },
                "autosave" => {
                    let autosave = value
                        .get::<bool>()
                        .expect("The value needs to be of type `bool`.");

                    self.autosave.replace(autosave);

                    if autosave {
                        self.update_autosave_handler(obj);
                    } else if let Some(autosave_source_id) =
                        self.autosave_source_id.borrow_mut().take()
                    {
                        autosave_source_id.remove();
                    }
                },
                "autosave-interval-secs" => {
                    let autosave_interval_secs = value
                        .get::<u32>()
                        .expect("The value needs to be of type `u32`.");

                    self.autosave_interval_secs.replace(autosave_interval_secs);

                    if self.autosave.get() {
                        self.update_autosave_handler(obj);
                    }
                },
                "righthanded" => {
                    let righthanded = value
                        .get::<bool>()
                        .expect("The value needs to be of type `bool`.");

                    self.righthanded.replace(righthanded);

                    self.handle_righthanded_property(righthanded);
                },
                "permanently-hide-canvas-scrollbars" => {
                    let permanently_hide_canvas_scrollbars = value
                        .get::<bool>()
                        .expect("The value needs to be of type `bool`.");

                    self.permanently_hide_canvas_scrollbars
                        .replace(permanently_hide_canvas_scrollbars);

                    if permanently_hide_canvas_scrollbars {
                        self.canvas_scroller.hscrollbar().set_visible(false);
                        self.canvas_scroller.vscrollbar().set_visible(false);
                    } else {
                        self.canvas_scroller.hscrollbar().set_visible(true);
                        self.canvas_scroller.vscrollbar().set_visible(true);
                    }
                },
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for RnoteAppWindow {}

    impl WindowImpl for RnoteAppWindow {
        // Save window state right before the window will be closed
        fn close_request(&self, obj: &Self::Type) -> Inhibit {
            // Save current doc
            if obj.unsaved_changes() {
                dialogs::dialog_quit_save(obj);
            } else {
                obj.close_force();
            }

            // Inhibit (Overwrite) the default handler. This handler is then responsible for destoying the window.
            Inhibit(true)
        }
    }

    impl ApplicationWindowImpl for RnoteAppWindow {}
    impl AdwWindowImpl for RnoteAppWindow {}
    impl AdwApplicationWindowImpl for RnoteAppWindow {}

    impl RnoteAppWindow {
        fn update_autosave_handler(&self, obj: &super::RnoteAppWindow) {
            if let Some(removed_id) = self.autosave_source_id.borrow_mut().replace(glib::source::timeout_add_seconds_local(self.autosave_interval_secs.get(), clone!(@strong obj as appwindow => @default-return glib::source::Continue(false), move || {
                if let Some(output_file) = appwindow.canvas().output_file() {
                    glib::MainContext::default().spawn_local(clone!(@strong appwindow => async move {
                        if let Err(e) = appwindow.save_document_to_file(&output_file).await {
                            appwindow.canvas().set_output_file(None);

                            log::error!("saving document failed with error `{}`", e);
                            adw::prelude::ActionGroupExt::activate_action(&appwindow, "error-toast", Some(&gettext("Saving document failed.").to_variant()));
                        }
                    }));
                }

                glib::source::Continue(true)
            }))) {
                removed_id.remove();
            }
        }

        // Setting up the sidebar flap
        fn setup_flap(&self, obj: &super::RnoteAppWindow) {
            let flap = self.flap.get();
            let flap_box = self.flap_box.get();
            let flap_resizer = self.flap_resizer.get();
            let flap_resizer_box = self.flap_resizer_box.get();
            let workspace_headerbar = self.flap_header.get();
            let flapreveal_toggle = self.flapreveal_toggle.get();

            flap.set_locked(true);
            flap.set_fold_policy(adw::FlapFoldPolicy::Auto);

            let expanded_revealed = Rc::new(Cell::new(flap.reveals_flap()));

            self.flapreveal_toggle
                .bind_property("active", &flap, "reveal-flap")
                .flags(
                    glib::BindingFlags::SYNC_CREATE
                        | glib::BindingFlags::BIDIRECTIONAL,
                )
                .build();

            self.flapreveal_toggle.connect_toggled(
                clone!(@weak flap, @strong expanded_revealed => move |flapreveal_toggle| {
                    flap.set_reveal_flap(flapreveal_toggle.is_active());
                    if !flap.is_folded() {
                        expanded_revealed.set(flapreveal_toggle.is_active());
                    }
                }),
            );

            self.flap
                .connect_folded_notify(clone!(@weak obj as appwindow, @strong expanded_revealed, @weak flapreveal_toggle, @weak workspace_headerbar => move |flap| {
                    if appwindow.mainheader().appmenu().parent().is_some() {
                        appwindow.mainheader().appmenu().unparent();
                    }

                    if flap.reveals_flap() && !flap.is_folded() {
                        // Set visible before appending, to avoid allocation glitch
                        appwindow.flap_menus_box().set_visible(true);
                        appwindow.flap_close_button().set_visible(false);
                        appwindow.flap_menus_box().append(&appwindow.mainheader().appmenu());
                    } else {
                        appwindow.flap_menus_box().set_visible(false);
                        appwindow.flap_close_button().set_visible(true);
                        appwindow.mainheader().menus_box().append(&appwindow.mainheader().appmenu());
                    }

                    if flap.is_folded() {
                        flapreveal_toggle.set_active(false);
                    } else if expanded_revealed.get() || flap.reveals_flap() {
                        expanded_revealed.set(true);
                        flapreveal_toggle.set_active(true);
                    }

                    if flap.flap_position() == PackType::Start {
                        workspace_headerbar.set_show_start_title_buttons(flap.reveals_flap());
                        workspace_headerbar.set_show_end_title_buttons(false);
                    } else if flap.flap_position() == PackType::End {
                        workspace_headerbar.set_show_start_title_buttons(false);
                        workspace_headerbar.set_show_end_title_buttons(flap.reveals_flap());
                    }
                }));

            self.flap
                .connect_reveal_flap_notify(clone!(@weak obj as appwindow, @weak workspace_headerbar => move |flap| {
                    if appwindow.mainheader().appmenu().parent().is_some() {
                        appwindow.mainheader().appmenu().unparent();
                    }

                    if flap.reveals_flap() && !flap.is_folded() {
                        appwindow.flap_menus_box().set_visible(true);
                        appwindow.flap_close_button().set_visible(false);
                        appwindow.flap_menus_box().append(&appwindow.mainheader().appmenu());
                    } else {
                        appwindow.flap_menus_box().set_visible(false);
                        appwindow.flap_close_button().set_visible(true);
                        appwindow.mainheader().menus_box().append(&appwindow.mainheader().appmenu());
                    }

                    if flap.flap_position() == PackType::Start {
                        workspace_headerbar.set_show_start_title_buttons(flap.reveals_flap());
                        workspace_headerbar.set_show_end_title_buttons(false);
                    } else if flap.flap_position() == PackType::End {
                        workspace_headerbar.set_show_start_title_buttons(false);
                        workspace_headerbar.set_show_end_title_buttons(flap.reveals_flap());
                    }
                }));

            self.flap.connect_flap_position_notify(
                clone!(@weak flap_resizer_box, @weak flap_resizer, @weak flap_box, @weak workspace_headerbar, @strong expanded_revealed, @weak obj as appwindow => move |flap| {
                    if flap.flap_position() == PackType::Start {
                        workspace_headerbar.set_show_start_title_buttons(flap.reveals_flap());
                        workspace_headerbar.set_show_end_title_buttons(false);

                        flap_resizer_box.reorder_child_after(&flap_resizer, Some(&flap_box));

                        appwindow.flap_header().remove(&appwindow.flap_close_button());
                        appwindow.flap_header().pack_end(&appwindow.flap_close_button());
                        appwindow.flap_close_button().set_icon_name("left-symbolic");
                    } else if flap.flap_position() == PackType::End {
                        workspace_headerbar.set_show_start_title_buttons(false);
                        workspace_headerbar.set_show_end_title_buttons(flap.reveals_flap());

                        flap_resizer_box.reorder_child_after(&flap_box, Some(&flap_resizer));

                        appwindow.flap_header().remove(&appwindow.flap_close_button());
                        appwindow.flap_header().pack_start(&appwindow.flap_close_button());
                        appwindow.flap_close_button().set_icon_name("right-symbolic");
                    }
                }),
            );

            // Resizing the flap contents
            let resizer_drag_gesture = GestureDrag::builder()
                .name("resizer_drag_gesture")
                .propagation_phase(PropagationPhase::Capture)
                .build();
            self.flap_resizer.add_controller(&resizer_drag_gesture);

            // hack to stop resizing when it is switching from non-folded to folded or vice versa (else gtk crashes)
            let prev_folded = Rc::new(Cell::new(self.flap.get().is_folded()));

            resizer_drag_gesture.connect_drag_begin(clone!(@strong prev_folded, @weak flap, @weak flap_box => move |_resizer_drag_gesture, _x , _y| {
                    prev_folded.set(flap.is_folded());
            }));

            resizer_drag_gesture.connect_drag_update(clone!(@weak obj, @strong prev_folded, @weak flap, @weak flap_box, @weak flapreveal_toggle => move |_resizer_drag_gesture, x , _y| {
                if flap.is_folded() == prev_folded.get() {
                    // Set BEFORE new width request
                    prev_folded.set(flap.is_folded());

                    let new_width = if flap.flap_position() == PackType::Start {
                        flap_box.width() + x.ceil() as i32
                    } else {
                        flap_box.width() - x.floor() as i32
                    };

                    if new_width > 0 && new_width < obj.mainheader().width() - super::RnoteAppWindow::FLAP_FOLDED_RESIZE_MARGIN as i32 {
                        flap_box.set_width_request(new_width);
                    }
                } else if flap.is_folded() {
                    flapreveal_toggle.set_active(true);
                }
            }));

            self.flap_resizer.set_cursor(
                gdk::Cursor::from_name(
                    "col-resize",
                    gdk::Cursor::from_name("default", None).as_ref(),
                )
                .as_ref(),
            );

            self.flap_close_button.get().connect_clicked(
                clone!(@weak obj => move |_flap_close_button| {
                    if obj.flap().reveals_flap() && obj.flap().is_folded() {
                        obj.flap().set_reveal_flap(false);
                    }
                }),
            );
        }

        fn handle_righthanded_property(&self, righthanded: bool) {
            let appwindow = self.instance();

            if righthanded {
                appwindow.flap().set_flap_position(PackType::Start);
                appwindow.main_grid().remove(&appwindow.sidebar_grid());
                appwindow.main_grid().remove(&appwindow.sidebar_sep());
                appwindow
                    .main_grid()
                    .remove(&appwindow.narrow_pens_toggles_revealer());
                appwindow.main_grid().remove(&appwindow.canvas_box());
                appwindow.main_grid().attach(
                    &appwindow.sidebar_grid(),
                    0,
                    1,
                    1,
                    2,
                );
                appwindow.main_grid().attach(
                    &appwindow.sidebar_sep(),
                    1,
                    1,
                    1,
                    2,
                );
                appwindow.main_grid().attach(
                    &appwindow.narrow_pens_toggles_revealer(),
                    2,
                    1,
                    1,
                    1,
                );
                appwindow.main_grid().attach(
                    &appwindow.canvas_box(),
                    2,
                    2,
                    1,
                    1,
                );
                appwindow.canvas_quickactions_box().set_halign(Align::End);
                appwindow
                    .mainheader()
                    .appmenu()
                    .righthanded_toggle()
                    .set_active(true);
                appwindow
                    .mainheader()
                    .headerbar()
                    .remove(&appwindow.mainheader().pens_toggles_squeezer());
                appwindow.mainheader().headerbar().pack_start(
                    &appwindow.mainheader().pens_toggles_squeezer(),
                );
                appwindow
                    .workspacebrowser()
                    .grid()
                    .remove(&appwindow.workspacebrowser().workspace_bar());
                appwindow
                    .workspacebrowser()
                    .grid()
                    .remove(&appwindow.workspacebrowser().files_scroller());
                appwindow.workspacebrowser().grid().attach(
                    &appwindow.workspacebrowser().workspace_bar(),
                    0,
                    0,
                    1,
                    1,
                );
                appwindow.workspacebrowser().grid().attach(
                    &appwindow.workspacebrowser().files_scroller(),
                    2,
                    0,
                    1,
                    1,
                );
                appwindow
                    .workspacebrowser()
                    .files_scroller()
                    .set_window_placement(CornerType::TopRight);
                appwindow
                    .workspacebrowser()
                    .workspace_scroller()
                    .set_window_placement(CornerType::TopRight);

                appwindow
                    .canvas_scroller()
                    .set_window_placement(CornerType::BottomRight);
                appwindow
                    .sidebar_scroller()
                    .set_window_placement(CornerType::TopRight);
                appwindow
                    .settings_panel()
                    .settings_scroller()
                    .set_window_placement(CornerType::TopRight);
                appwindow
                    .penssidebar()
                    .brush_page()
                    .brushstyle_preset()
                    .set_direction(ArrowType::Right);
                appwindow
                    .penssidebar()
                    .brush_page()
                    .brushconfig_menubutton()
                    .set_direction(ArrowType::Right);
                appwindow
                    .penssidebar()
                    .brush_page()
                    .brushstyle_menubutton()
                    .set_direction(ArrowType::Right);
                appwindow
                    .penssidebar()
                    .brush_page()
                    .colorpicker()
                    .set_property("position", PositionType::Left.to_value());
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .stroke_colorpicker()
                    .set_property("position", PositionType::Left.to_value());
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .fill_colorpicker()
                    .set_property("position", PositionType::Left.to_value());
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .shaperstyle_menubutton()
                    .set_direction(ArrowType::Right);
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .shapeconfig_menubutton()
                    .set_direction(ArrowType::Right);
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .shapebuildertype_menubutton()
                    .set_direction(ArrowType::Right);
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .constraint_menubutton()
                    .set_direction(ArrowType::Right);
                appwindow
                    .penssidebar()
                    .typewriter_page()
                    .colorpicker()
                    .set_property("position", PositionType::Left.to_value());
            } else {
                appwindow.flap().set_flap_position(PackType::End);
                appwindow.main_grid().remove(&appwindow.canvas_box());
                appwindow
                    .main_grid()
                    .remove(&appwindow.narrow_pens_toggles_revealer());
                appwindow.main_grid().remove(&appwindow.sidebar_sep());
                appwindow.main_grid().remove(&appwindow.sidebar_grid());
                appwindow.main_grid().attach(
                    &appwindow.canvas_box(),
                    0,
                    2,
                    1,
                    1,
                );
                appwindow.main_grid().attach(
                    &appwindow.narrow_pens_toggles_revealer(),
                    0,
                    1,
                    1,
                    1,
                );
                appwindow.main_grid().attach(
                    &appwindow.sidebar_sep(),
                    1,
                    1,
                    1,
                    2,
                );
                appwindow.main_grid().attach(
                    &appwindow.sidebar_grid(),
                    2,
                    1,
                    1,
                    2,
                );
                appwindow.canvas_quickactions_box().set_halign(Align::Start);
                appwindow
                    .mainheader()
                    .appmenu()
                    .lefthanded_toggle()
                    .set_active(true);
                appwindow
                    .mainheader()
                    .headerbar()
                    .remove(&appwindow.mainheader().pens_toggles_squeezer());
                appwindow
                    .mainheader()
                    .headerbar()
                    .pack_end(&appwindow.mainheader().pens_toggles_squeezer());
                appwindow
                    .workspacebrowser()
                    .grid()
                    .remove(&appwindow.workspacebrowser().files_scroller());
                appwindow
                    .workspacebrowser()
                    .grid()
                    .remove(&appwindow.workspacebrowser().workspace_bar());
                appwindow.workspacebrowser().grid().attach(
                    &appwindow.workspacebrowser().files_scroller(),
                    0,
                    0,
                    1,
                    1,
                );
                appwindow.workspacebrowser().grid().attach(
                    &appwindow.workspacebrowser().workspace_bar(),
                    2,
                    0,
                    1,
                    1,
                );
                appwindow
                    .workspacebrowser()
                    .files_scroller()
                    .set_window_placement(CornerType::TopLeft);
                appwindow
                    .workspacebrowser()
                    .workspace_scroller()
                    .set_window_placement(CornerType::TopLeft);

                appwindow
                    .canvas_scroller()
                    .set_window_placement(CornerType::BottomLeft);
                appwindow
                    .sidebar_scroller()
                    .set_window_placement(CornerType::TopLeft);
                appwindow
                    .settings_panel()
                    .settings_scroller()
                    .set_window_placement(CornerType::TopLeft);
                appwindow
                    .penssidebar()
                    .brush_page()
                    .brushstyle_preset()
                    .set_direction(ArrowType::Left);
                appwindow
                    .penssidebar()
                    .brush_page()
                    .brushconfig_menubutton()
                    .set_direction(ArrowType::Left);
                appwindow
                    .penssidebar()
                    .brush_page()
                    .brushstyle_menubutton()
                    .set_direction(ArrowType::Left);
                appwindow
                    .penssidebar()
                    .brush_page()
                    .colorpicker()
                    .set_property("position", PositionType::Right.to_value());
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .stroke_colorpicker()
                    .set_property("position", PositionType::Right.to_value());
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .fill_colorpicker()
                    .set_property("position", PositionType::Right.to_value());
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .shaperstyle_menubutton()
                    .set_direction(ArrowType::Left);
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .shapeconfig_menubutton()
                    .set_direction(ArrowType::Left);
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .shapebuildertype_menubutton()
                    .set_direction(ArrowType::Left);
                appwindow
                    .penssidebar()
                    .shaper_page()
                    .constraint_menubutton()
                    .set_direction(ArrowType::Left);
                appwindow
                    .penssidebar()
                    .typewriter_page()
                    .colorpicker()
                    .set_property("position", PositionType::Right.to_value());
            }
        }
    }
}

glib::wrapper! {
    pub struct RnoteAppWindow(ObjectSubclass<imp::RnoteAppWindow>)
        @extends gtk4::Widget, gtk4::Window, adw::Window, gtk4::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

pub static OUTPUT_FILE_NEW_TITLE: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| gettext("New Document"));
pub static OUTPUT_FILE_NEW_SUBTITLE: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| gettext("Draft"));

impl RnoteAppWindow {
    const AUTOSAVE_INTERVAL_DEFAULT: u32 = 120;

    const FLAP_FOLDED_RESIZE_MARGIN: u32 = 64;

    pub fn new(app: &Application) -> Self {
        glib::Object::new(&[("application", app)])
            .expect("Failed to create `RnoteAppWindow`.")
    }

    /// Called to close the window
    pub fn close_force(&self) {
        // Saving all state
        if let Err(err) = self.save_to_settings() {
            log::error!(
                "Failed to save appwindow to settings, with Err `{}`",
                &err
            );
        }

        // Closing the state tasks channel receiver
        if let Err(e) = self
            .canvas()
            .engine()
            .borrow()
            .tasks_tx()
            .unbounded_send(EngineTask::Quit)
        {
            log::error!(
                "failed to send StateTask::Quit on store tasks_tx, Err {}",
                e
            );
        }

        self.destroy();
    }

    pub fn app(&self) -> RnoteApp {
        self.application().unwrap().downcast::<RnoteApp>().unwrap()
    }

    pub fn app_settings(&self) -> gio::Settings {
        self.imp().app_settings.clone()
    }

    pub fn filechoosernative(&self) -> Rc<RefCell<Option<FileChooserNative>>> {
        self.imp().filechoosernative.clone()
    }

    pub fn unsaved_changes(&self) -> bool {
        self.property::<bool>("unsaved-changes")
    }

    pub fn set_unsaved_changes(&self, unsaved_changes: bool) {
        self.set_property("unsaved-changes", unsaved_changes.to_value());
    }

    pub fn autosave(&self) -> bool {
        self.property::<bool>("autosave")
    }

    pub fn set_autosave(&self, autosave: bool) {
        self.set_property("autosave", autosave.to_value());
    }

    pub fn autosave_interval_secs(&self) -> u32 {
        self.property::<u32>("autosave-interval-secs")
    }

    pub fn set_autosave_interval_secs(&self, autosave_interval_secs: u32) {
        self.set_property(
            "autosave-interval-secs",
            autosave_interval_secs.to_value(),
        );
    }

    pub fn righthanded(&self) -> bool {
        self.property::<bool>("righthanded")
    }

    pub fn set_righthanded(&self, righthanded: bool) {
        self.set_property("righthanded", righthanded.to_value());
    }

    pub fn permanently_hide_canvas_scrollbars(&self) -> bool {
        self.property::<bool>("permanently_hide_canvas_scrollbars")
    }

    pub fn set_permanently_hide_canvas_scrollbars(
        &self,
        permanently_hide_canvas_scrollbars: bool,
    ) {
        self.set_property(
            "permanently_hide_canvas_scrollbars",
            permanently_hide_canvas_scrollbars.to_value(),
        );
    }

    pub fn canvas_touch_drag_gesture(&self) -> GestureDrag {
        self.imp().canvas_touch_drag_gesture.clone()
    }

    pub fn canvas_drag_empty_area_gesture(&self) -> GestureDrag {
        self.imp().canvas_drag_empty_area_gesture.clone()
    }

    pub fn canvas_zoom_gesture(&self) -> GestureZoom {
        self.imp().canvas_zoom_gesture.clone()
    }

    pub fn canvas_zoom_scroll_controller(&self) -> EventControllerScroll {
        self.imp().canvas_zoom_scroll_controller.clone()
    }

    pub fn canvas_mouse_drag_middle_gesture(&self) -> GestureDrag {
        self.imp().canvas_mouse_drag_middle_gesture.clone()
    }

    pub fn toast_overlay(&self) -> adw::ToastOverlay {
        self.imp().toast_overlay.get()
    }

    pub fn main_grid(&self) -> Grid {
        self.imp().main_grid.get()
    }

    pub fn canvas_box(&self) -> gtk4::Box {
        self.imp().canvas_box.get()
    }

    pub fn canvas_quickactions_box(&self) -> gtk4::Box {
        self.imp().canvas_quickactions_box.get()
    }

    pub fn canvas_fixedsize_quickactions_revealer(&self) -> Revealer {
        self.imp().canvas_fixedsize_quickactions_revealer.get()
    }

    pub fn undo_button(&self) -> Button {
        self.imp().undo_button.get()
    }

    pub fn redo_button(&self) -> Button {
        self.imp().redo_button.get()
    }

    pub fn canvas_progressbar(&self) -> ProgressBar {
        self.imp().canvas_progressbar.get()
    }

    pub fn canvas_scroller(&self) -> ScrolledWindow {
        self.imp().canvas_scroller.get()
    }

    pub fn canvas(&self) -> RnoteCanvas {
        self.imp().canvas.get()
    }

    pub fn settings_panel(&self) -> SettingsPanel {
        self.imp().settings_panel.get()
    }

    pub fn sidebar_scroller(&self) -> ScrolledWindow {
        self.imp().sidebar_scroller.get()
    }

    pub fn sidebar_grid(&self) -> Grid {
        self.imp().sidebar_grid.get()
    }

    pub fn sidebar_sep(&self) -> Separator {
        self.imp().sidebar_sep.get()
    }

    pub fn flap_box(&self) -> gtk4::Box {
        self.imp().flap_box.get()
    }

    pub fn flap_header(&self) -> adw::HeaderBar {
        self.imp().flap_header.get()
    }

    pub fn workspacebrowser(&self) -> WorkspaceBrowser {
        self.imp().workspacebrowser.get()
    }

    pub fn flap(&self) -> adw::Flap {
        self.imp().flap.get()
    }

    pub fn flapreveal_toggle(&self) -> ToggleButton {
        self.imp().flapreveal_toggle.get()
    }

    pub fn flap_menus_box(&self) -> Box {
        self.imp().flap_menus_box.get()
    }

    pub fn flap_close_button(&self) -> Button {
        self.imp().flap_close_button.get()
    }

    pub fn flap_stack(&self) -> adw::ViewStack {
        self.imp().flap_stack.get()
    }

    pub fn mainheader(&self) -> MainHeader {
        self.imp().mainheader.get()
    }

    pub fn narrow_pens_toggles_revealer(&self) -> Revealer {
        self.imp().narrow_pens_toggles_revealer.get()
    }

    pub fn narrow_brush_toggle(&self) -> ToggleButton {
        self.imp().narrow_brush_toggle.get()
    }

    pub fn narrow_shaper_toggle(&self) -> ToggleButton {
        self.imp().narrow_shaper_toggle.get()
    }

    pub fn narrow_typewriter_toggle(&self) -> ToggleButton {
        self.imp().narrow_typewriter_toggle.get()
    }

    pub fn narrow_eraser_toggle(&self) -> ToggleButton {
        self.imp().narrow_eraser_toggle.get()
    }

    pub fn narrow_selector_toggle(&self) -> ToggleButton {
        self.imp().narrow_selector_toggle.get()
    }

    pub fn narrow_tools_toggle(&self) -> ToggleButton {
        self.imp().narrow_tools_toggle.get()
    }

    pub fn penssidebar(&self) -> PensSideBar {
        self.imp().penssidebar.get()
    }

    // Must be called after application is associated with it else it fails
    pub fn init(&self) {
        self.imp().workspacebrowser.get().init(self);
        self.imp().settings_panel.get().init(self);
        self.imp().mainheader.get().init(self);
        self.imp().mainheader.get().canvasmenu().init(self);
        self.imp().mainheader.get().appmenu().init(self);
        self.imp().penssidebar.get().init(self);
        self.imp().penssidebar.get().brush_page().init(self);
        self.imp().penssidebar.get().shaper_page().init(self);
        self.imp().penssidebar.get().typewriter_page().init(self);
        self.imp().penssidebar.get().eraser_page().init(self);
        self.imp().penssidebar.get().selector_page().init(self);
        self.imp().penssidebar.get().tools_page().init(self);
        self.imp().canvas.get().init(self);

        // add icon theme resource path because automatic lookup does not work in the devel build.
        let app_icon_theme = IconTheme::for_display(&self.display());
        app_icon_theme.add_resource_path(
            (String::from(config::APP_IDPATH) + "icons").as_str(),
        );

        self.setup_input();

        // actions and settings AFTER widget inits
        self.setup_actions();
        self.setup_action_accels();
        self.setup_settings_binds();

        // Load settings
        self.load_settings();

        // Loading in input file, if Some
        if let Some(input_file) = self.app().input_file() {
            self.open_file_w_dialogs(&input_file, None);
        }

        // Initial titles
        self.update_titles_for_file(None);
    }

    pub fn canvas_touch_drag_gesture_enable(&self, enable: bool) {
        if enable {
            self.imp()
                .canvas_touch_drag_gesture
                .set_propagation_phase(PropagationPhase::Bubble);
        } else {
            self.imp()
                .canvas_touch_drag_gesture
                .set_propagation_phase(PropagationPhase::None);
        }
    }

    pub fn canvas_drag_empty_area_gesture_enable(&self, enable: bool) {
        if enable {
            self.imp()
                .canvas_drag_empty_area_gesture
                .set_propagation_phase(PropagationPhase::Bubble);
        } else {
            self.imp()
                .canvas_drag_empty_area_gesture
                .set_propagation_phase(PropagationPhase::None);
        }
    }

    pub fn canvas_zoom_gesture_enable(&self, enable: bool) {
        if enable {
            self.imp()
                .canvas_zoom_gesture
                .set_propagation_phase(PropagationPhase::Capture);
        } else {
            self.imp()
                .canvas_zoom_gesture
                .set_propagation_phase(PropagationPhase::None);
        }
    }

    pub fn canvas_zoom_scroll_controller_enable(&self, enable: bool) {
        if enable {
            self.imp()
                .canvas_zoom_scroll_controller
                .set_propagation_phase(PropagationPhase::Bubble);
        } else {
            self.imp()
                .canvas_zoom_scroll_controller
                .set_propagation_phase(PropagationPhase::None);
        }
    }

    pub fn canvas_mouse_drag_middle_gesture_enable(&self, enable: bool) {
        if enable {
            self.imp()
                .canvas_mouse_drag_middle_gesture
                .set_propagation_phase(PropagationPhase::Bubble);
        } else {
            self.imp()
                .canvas_mouse_drag_middle_gesture
                .set_propagation_phase(PropagationPhase::None);
        }
    }

    pub fn setup_input(&self) {
        // Gesture Grouping
        self.imp()
            .canvas_mouse_drag_middle_gesture
            .group_with(&self.imp().canvas_touch_drag_gesture);
        self.imp()
            .canvas_drag_empty_area_gesture
            .group_with(&self.imp().canvas_touch_drag_gesture);

        // zoom scrolling with <ctrl> + scroll
        {
            self.imp().canvas_zoom_scroll_controller.connect_scroll(clone!(@weak self as appwindow => @default-return Inhibit(false), move |zoom_scroll_controller, _dx, dy| {
                if zoom_scroll_controller.current_event_state() == gdk::ModifierType::CONTROL_MASK {
                    let new_zoom = appwindow.canvas().engine().borrow().camera.total_zoom() * (1.0 - dy * RnoteCanvas::ZOOM_STEP);

                    let current_doc_center = appwindow.canvas().current_center_on_doc();
                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "zoom-to-value", Some(&new_zoom.to_variant()));
                    appwindow.canvas().center_around_coord_on_doc(current_doc_center);

                    // Stop event propagation
                    Inhibit(true)
                } else {
                    Inhibit(false)
                }
            }));
        }

        // Drag canvas with touch gesture
        {
            let touch_drag_start = Rc::new(Cell::new(na::vector![0.0, 0.0]));

            self.imp().canvas_touch_drag_gesture.connect_drag_begin(clone!(@strong touch_drag_start, @weak self as appwindow => move |_canvas_touch_drag_gesture, _x, _y| {
                touch_drag_start.set(na::vector![
                    appwindow.canvas().hadjustment().unwrap().value(),
                    appwindow.canvas().vadjustment().unwrap().value()
                ]);
            }));
            self.imp().canvas_touch_drag_gesture.connect_drag_update(clone!(@strong touch_drag_start, @weak self as appwindow => move |_canvas_touch_drag_gesture, x, y| {
                let new_adj_values = touch_drag_start.get() - na::vector![x,y];

                appwindow.canvas().update_camera_offset(new_adj_values);
            }));

            self.imp().canvas_touch_drag_gesture.connect_drag_end(
                clone!(@weak self as appwindow => move |_canvas_touch_drag_gesture, _x, _y| {
                    appwindow.canvas().update_engine_rendering();
                }),
            );
        }

        // Move Canvas with middle mouse button
        {
            let mouse_drag_start = Rc::new(Cell::new(na::vector![0.0, 0.0]));

            self.imp().canvas_mouse_drag_middle_gesture.connect_drag_begin(clone!(@strong mouse_drag_start, @weak self as appwindow => move |_canvas_mouse_drag_middle_gesture, _x, _y| {
                mouse_drag_start.set(na::vector![
                    appwindow.canvas().hadjustment().unwrap().value(),
                    appwindow.canvas().vadjustment().unwrap().value()
                ]);
            }));
            self.imp().canvas_mouse_drag_middle_gesture.connect_drag_update(clone!(@strong mouse_drag_start, @weak self as appwindow => move |_canvas_mouse_drag_gesture, x, y| {
                let new_adj_values = mouse_drag_start.get() - na::vector![x,y];

                appwindow.canvas().update_camera_offset(new_adj_values);
            }));

            self.imp().canvas_mouse_drag_middle_gesture.connect_drag_end(
                clone!(@weak self as appwindow => move |_canvas_mouse_drag_middle_gesture, _x, _y| {
                    appwindow.canvas().update_engine_rendering();
                }),
            );
        }

        // Move Canvas by dragging in empty area
        {
            let mouse_drag_empty_area_start =
                Rc::new(Cell::new(na::vector![0.0, 0.0]));

            self.imp().canvas_drag_empty_area_gesture.connect_drag_begin(clone!(@strong mouse_drag_empty_area_start, @weak self as appwindow => move |_, _x, _y| {
                mouse_drag_empty_area_start.set(na::vector![
                    appwindow.canvas().hadjustment().unwrap().value(),
                    appwindow.canvas().vadjustment().unwrap().value()
                ]);
            }));
            self.imp().canvas_drag_empty_area_gesture.connect_drag_update(clone!(@strong mouse_drag_empty_area_start, @weak self as appwindow => move |_, x, y| {
                let new_adj_values = mouse_drag_empty_area_start.get() - na::vector![x,y];

                appwindow.canvas().update_camera_offset(new_adj_values);
            }));

            self.imp().canvas_drag_empty_area_gesture.connect_drag_end(
                clone!(@weak self as appwindow => move |_, _x, _y| {
                    appwindow.canvas().update_engine_rendering();
                }),
            );
        }

        // Canvas gesture zooming with dragging
        {
            let prev_scale = Rc::new(Cell::new(1_f64));
            let zoom_begin = Rc::new(Cell::new(1_f64));
            let new_zoom = Rc::new(Cell::new(1.0));
            let bbcenter_begin: Rc<Cell<Option<na::Vector2<f64>>>> =
                Rc::new(Cell::new(None));
            let adjs_begin = Rc::new(Cell::new(na::vector![0.0, 0.0]));

            self.imp().canvas_zoom_gesture.connect_begin(clone!(
                @strong zoom_begin,
                @strong new_zoom,
                @strong prev_scale,
                @strong bbcenter_begin,
                @strong adjs_begin,
                @weak self as appwindow => move |canvas_zoom_gesture, _event_sequence| {
                    let current_zoom = appwindow.canvas().engine().borrow().camera.zoom();
                    canvas_zoom_gesture.set_state(EventSequenceState::Claimed);

                    zoom_begin.set(current_zoom);
                    new_zoom.set(current_zoom);
                    prev_scale.set(1.0);

                    bbcenter_begin.set(canvas_zoom_gesture.bounding_box_center().map(|coords| na::vector![coords.0, coords.1]));
                    adjs_begin.set(na::vector![appwindow.canvas().hadjustment().unwrap().value(), appwindow.canvas().vadjustment().unwrap().value()]);
            }));

            self.imp().canvas_zoom_gesture.connect_scale_changed(clone!(
                @strong zoom_begin,
                @strong new_zoom,
                @strong prev_scale,
                @strong bbcenter_begin,
                @strong adjs_begin,
                @weak self as appwindow => move |canvas_zoom_gesture, scale| {
                    if zoom_begin.get() * scale <= Camera::ZOOM_MAX && zoom_begin.get() * scale >= Camera::ZOOM_MIN {
                        new_zoom.set(zoom_begin.get() * scale);
                        prev_scale.set(scale);
                    }

                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "zoom-to-value", Some(&new_zoom.get().to_variant()));

                    if let Some(bbcenter_current) = canvas_zoom_gesture.bounding_box_center().map(|coords| na::vector![coords.0, coords.1]) {
                        let bbcenter_begin = if let Some(bbcenter_begin) = bbcenter_begin.get() {
                            bbcenter_begin
                        } else {
                            // Set the center if not set by gesture begin handler
                            bbcenter_begin.set(Some(bbcenter_current));
                            bbcenter_current
                        };

                        let bbcenter_delta = bbcenter_current - bbcenter_begin * prev_scale.get();
                        let new_adj_values = adjs_begin.get() * prev_scale.get() - bbcenter_delta;

                        appwindow.canvas().update_camera_offset(new_adj_values);
                    }
            }));

            self.imp().canvas_zoom_gesture.connect_cancel(
                clone!(@strong new_zoom, @strong bbcenter_begin, @weak self as appwindow => move |canvas_zoom_gesture, _event_sequence| {
                    bbcenter_begin.set(None);

                    appwindow.canvas().update_engine_rendering();

                    canvas_zoom_gesture.set_state(EventSequenceState::Denied);
                }),
            );

            self.imp().canvas_zoom_gesture.connect_end(
                clone!(@strong new_zoom, @strong bbcenter_begin, @weak self as appwindow => move |canvas_zoom_gesture, _event_sequence| {
                    adw::prelude::ActionGroupExt::activate_action(&appwindow, "zoom-to-value", Some(&new_zoom.get().to_variant()));

                    bbcenter_begin.set(None);

                    appwindow.canvas().update_engine_rendering();

                    canvas_zoom_gesture.set_state(EventSequenceState::Denied);
                }),
            );
        }
    }

    // Returns true if the flags indicate that any loop that handles the flags should be quit. (usually an async event loop)
    pub fn handle_widget_flags(&self, widget_flags: WidgetFlags) -> bool {
        if widget_flags.redraw {
            self.canvas().queue_draw();
        }
        if widget_flags.resize {
            self.canvas().queue_resize();
        }
        if widget_flags.refresh_ui {
            adw::prelude::ActionGroupExt::activate_action(
                self,
                "refresh-ui-for-engine",
                None,
            );
        }
        if widget_flags.indicate_changed_store {
            self.canvas().set_unsaved_changes(true);
            self.canvas().set_empty(false);
        }
        if widget_flags.update_view {
            let camera_offset = self.canvas().engine().borrow().camera.offset;
            // this updates the canvas adjustment values with the ones from the camera
            self.canvas().update_camera_offset(camera_offset);
        }
        if !self.permanently_hide_canvas_scrollbars() {
            if let Some(hide_scrollbars) = widget_flags.hide_scrollbars {
                if hide_scrollbars {
                    self.canvas_scroller()
                        .set_policy(PolicyType::Never, PolicyType::Never);
                } else {
                    self.canvas_scroller().set_policy(
                        PolicyType::Automatic,
                        PolicyType::Automatic,
                    );
                }
            }
        }
        if let Some(hide_undo) = widget_flags.hide_undo {
            self.undo_button().set_sensitive(!hide_undo);
        }
        if let Some(hide_redo) = widget_flags.hide_redo {
            self.redo_button().set_sensitive(!hide_redo);
        }
        if let Some(enable_text_preprocessing) =
            widget_flags.enable_text_preprocessing
        {
            self.canvas()
                .set_text_preprocessing(enable_text_preprocessing);
        }

        widget_flags.quit
    }

    pub fn save_engine_config(&self) -> anyhow::Result<()> {
        let engine_config =
            self.canvas().engine().borrow().save_engine_config()?;
        self.app_settings()
            .set_string("engine-config", engine_config.as_str())?;

        Ok(())
    }

    pub fn update_titles_for_file(&self, file: Option<&gio::File>) {
        let title: String = file
            .and_then(|f| f.basename())
            .map(|t| t.with_extension("").display().to_string())
            .unwrap_or_else(|| OUTPUT_FILE_NEW_TITLE.to_string());

        let subtitle: String = file
            .and_then(|f| Some(f.parent()?.path()?.display().to_string()))
            .unwrap_or_else(|| OUTPUT_FILE_NEW_SUBTITLE.to_string());

        self.set_title(Some(
            &(title.clone() + " - " + config::APP_NAME_CAPITALIZED),
        ));

        self.mainheader().main_title().set_title(&title);
        self.mainheader().main_title().set_subtitle(&subtitle);
    }

    pub fn start_pulsing_canvas_progressbar(&self) {
        const PROGRESS_BAR_PULSE_INTERVAL: std::time::Duration =
            std::time::Duration::from_millis(300);

        if let Some(old_pulse_source) = self.imp().progresspulse_source_id.replace(Some(glib::source::timeout_add_local(
            PROGRESS_BAR_PULSE_INTERVAL,
            clone!(@weak self as appwindow => @default-return glib::source::Continue(false), move || {
                appwindow.canvas_progressbar().pulse();

                glib::source::Continue(true)
            })),
        )) {
            old_pulse_source.remove();
        }
    }

    pub fn finish_canvas_progressbar(&self) {
        const PROGRESS_BAR_TIMEOUT_TIME: std::time::Duration =
            std::time::Duration::from_millis(300);

        if let Some(pulse_source) = self.imp().progresspulse_source_id.take() {
            pulse_source.remove();
        }

        self.canvas_progressbar().set_fraction(1.0);

        glib::source::timeout_add_local_once(
            PROGRESS_BAR_TIMEOUT_TIME,
            clone!(@weak self as appwindow => move || {
                appwindow.canvas_progressbar().set_fraction(0.0);
            }),
        );
    }

    pub fn abort_canvas_progressbar(&self) {
        if let Some(pulse_source) = self.imp().progresspulse_source_id.take() {
            pulse_source.remove();
        }

        self.canvas_progressbar().set_fraction(0.0);
    }
}
