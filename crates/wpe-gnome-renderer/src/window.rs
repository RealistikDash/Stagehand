use gtk4::prelude::*;
use wpe_gnome_core::model::ScalingMode;
use wpe_gnome_core::protocol::RendererArguments;
use wpe_gnome_core::protocol::RendererCommand;
use wpe_gnome_core::protocol::RendererContent;

use crate::content::Content;
use crate::content::ContentError;
use crate::stdin;
use crate::stdin::StdinEvent;
use crate::video::VideoContent;
use crate::web::WebContent;

pub fn build(
    application: &gtk4::Application,
    arguments: &RendererArguments,
) -> Result<(), ContentError> {
    let content: Box<dyn Content> = match &arguments.content {
        RendererContent::Video { file } => Box::new(VideoContent::new(file, arguments)?),
        RendererContent::Web { index } => Box::new(WebContent::new(index, arguments)?),
    };
    let window = gtk4::Window::builder()
        .application(application)
        .title(&arguments.label)
        .decorated(false)
        .resizable(false)
        .default_width(width(arguments))
        .default_height(height(arguments))
        .child(&content.widget())
        .build();
    window.set_size_request(width(arguments), height(arguments));
    window.add_css_class("wpe-gnome-renderer");
    install_css();
    window.present();
    tracing::info!(label = %arguments.label, size = %arguments.size, "Renderer window shown.");

    let commands = stdin::start();
    let application = application.clone();

    glib::spawn_future_local(async move {
        while let Ok(event) = commands.recv().await {
            match event {
                StdinEvent::Command(command) => apply(content.as_ref(), &command, &application),
                StdinEvent::Malformed(error) => {
                    tracing::warn!(error = %error, "Ignored malformed command.");
                }
                StdinEvent::Closed => {
                    tracing::info!("Daemon closed the command channel; exiting.");
                    application.quit();

                    break;
                }
            }
        }
    });

    Ok(())
}

fn apply(content: &dyn Content, command: &RendererCommand, application: &gtk4::Application) {
    match command {
        RendererCommand::Pause => content.pause(),
        RendererCommand::Resume => content.resume(),
        RendererCommand::SetAudio { muted, volume } => content.set_audio(*muted, *volume),
        RendererCommand::SetProperties { values } => content.set_properties(values),
        RendererCommand::Quit => application.quit(),
    }
}

fn install_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(".wpe-gnome-renderer { background-color: black; }");

    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn content_fit(scaling: ScalingMode) -> gtk4::ContentFit {
    match scaling {
        ScalingMode::Fill => gtk4::ContentFit::Cover,
        ScalingMode::Fit => gtk4::ContentFit::Contain,
        ScalingMode::Stretch => gtk4::ContentFit::Fill,
        ScalingMode::Centre => gtk4::ContentFit::ScaleDown,
    }
}

fn width(arguments: &RendererArguments) -> i32 {
    i32::try_from(arguments.size.width).unwrap_or(i32::MAX)
}

fn height(arguments: &RendererArguments) -> i32 {
    i32::try_from(arguments.size.height).unwrap_or(i32::MAX)
}
