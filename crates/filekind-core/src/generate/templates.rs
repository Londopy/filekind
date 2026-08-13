//! The minijinja environment and the render context.
//!
//! Templates are embedded at compile time. Their names deliberately contain no
//! dots: minijinja picks an autoescape mode from the file extension, and an
//! `.xml` template would silently HTML-escape everything. Escaping here is
//! explicit, per dialect, via filters — see [`super::escape`].

use serde::Serialize;
use std::sync::OnceLock;

use minijinja::{Environment, Value};

use crate::error::Result;
use crate::generate::{escape, GenerateOptions, Scope};
use crate::spec::{Container, Spec};
use crate::Platform;

macro_rules! templates {
    ($($name:literal => $file:literal),* $(,)?) => {
        const TEMPLATES: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../../templates/", $file)))),*
        ];
    };
}

templates! {
    "windows_register"    => "windows_register.reg.j2",
    "windows_unregister"  => "windows_unregister.reg.j2",
    "windows_inno"        => "windows_inno.iss.j2",
    "windows_nsis"        => "windows_nsis.nsh.j2",
    "linux_mime"          => "linux_mime.xml.j2",
    "linux_desktop"       => "linux_desktop.j2",
    "linux_install"       => "linux_install.sh.j2",
    "linux_uninstall"     => "linux_uninstall.sh.j2",
    "macos_fragment"      => "macos_fragment.plist.j2",
    "macos_app_plist"     => "macos_app_plist.j2",
    "macos_app_stub"      => "macos_app_stub.sh.j2",
    "universal_magic"     => "universal_magic.j2",
    "universal_readme"    => "universal_readme.md.j2",
    "deb_control"         => "deb_control.j2",
    "deb_postinst"        => "deb_postinst.sh.j2",
    "deb_postrm"          => "deb_postrm.sh.j2",
    "deb_build"           => "deb_build.sh.j2",
    "rpm_spec"            => "rpm_spec.j2",
}

fn environment() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        // Autoescape off everywhere. Five output dialects, none of them HTML.
        env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
        env.add_filter("reg", |s: String| escape::reg(&s));
        env.add_filter("xml", |s: String| escape::xml(&s));
        env.add_filter("inno", |s: String| escape::inno(&s));
        env.add_filter("nsis", |s: String| escape::nsis(&s));
        env.add_filter("dsk", |s: String| escape::desktop_string(&s));
        env.add_filter("dskl", |s: String| escape::desktop_list_item(&s));
        env.add_filter("sh", |s: String| escape::sh(&s));
        for (name, body) in TEMPLATES {
            env.add_template(name, body)
                .expect("embedded templates always compile");
        }
        env
    })
}

/// Render an embedded template. A failure here is a filekind bug, never bad
/// user input — every value in the context is already a `String`.
pub(crate) fn render(name: &str, ctx: &Ctx) -> Result<String> {
    let tmpl = environment().get_template(name)?;
    let out = tmpl.render(Value::from_serialize(ctx))?;
    // Templates are easier to read with trailing whitespace control left off;
    // normalise the result instead.
    let mut s: String = out
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    while s.ends_with('\n') {
        s.pop();
    }
    s.push('\n');
    Ok(s)
}

/// Everything a template can ask for, precomputed.
///
/// Flat and stringly-typed on purpose: templates should read values, not
/// compute them. Anything requiring a decision is decided here, in Rust, where
/// it can be tested.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Ctx {
    pub filekind_version: String,
    pub banner_semi: String,
    pub banner_hash: String,
    pub banner_plain: String,

    pub name: String,
    pub extension: String,
    pub dotted_ext: String,
    pub description: String,
    pub format_version: String,
    pub container: String,
    pub slug: String,

    pub mime: String,
    pub mime_icon_name: String,
    pub generic_icon: String,

    pub has_magic: bool,
    pub magic_freedesktop: String,
    pub magic_libmagic: String,
    pub magic_hex: String,
    pub magic_ascii: String,

    pub has_handler: bool,
    pub handler: String,
    pub windows_command: String,
    pub desktop_exec: String,

    pub progid: String,
    pub has_perceived_type: bool,
    pub perceived_type: String,
    pub has_icon_path: bool,
    pub icon_path: String,
    pub windows_default_icon: String,
    pub has_windows_default_icon: bool,
    pub verbs: Vec<Verb>,

    pub desktop_id: String,
    pub desktop_file: String,
    pub mime_file: String,
    pub categories: Vec<String>,
    pub has_generic_name: bool,
    pub generic_name: String,
    pub no_display: bool,

    pub uti: String,
    pub uti_conforms_to: Vec<String>,
    pub bundle_id: String,
    pub app_name: String,

    pub scope: String,
    pub is_system: bool,
    pub hive: String,
    pub hive_short: String,
    pub xdg_mode: String,
    pub data_dir_expr: String,

    pub has_icon: bool,
    pub hicolor_sizes: Vec<u32>,
    pub icon_warnings: Vec<String>,

    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Verb {
    pub name: String,
    pub command: String,
}

pub(crate) fn context(spec: &Spec, opts: &GenerateOptions) -> Ctx {
    let handler = spec.association.handler.clone().unwrap_or_default();
    let has_handler = !handler.trim().is_empty();

    let magic = spec.magic_bytes().ok().flatten().unwrap_or_default();
    let has_magic = !magic.is_empty();

    // DefaultIcon: an installer knows where it put the .ico; a loose .reg does
    // not. Prefer an explicit path, fall back to the handler's own icon
    // resource, and otherwise emit nothing rather than a path that will 404.
    let (windows_default_icon, has_windows_default_icon) = match &spec.windows.icon_path {
        Some(p) if !p.trim().is_empty() => (format!("{p},0"), true),
        _ if has_handler => (format!("{handler},0"), true),
        _ => (String::new(), false),
    };

    let generic_icon = match spec.format.container {
        Container::Zip => "package-x-generic",
        Container::Text => "text-x-generic",
        Container::Sqlite => "office-database",
        Container::Binary | Container::None => "application-x-generic",
    }
    .to_string();

    let mut targets = Vec::new();
    if spec.targets.windows {
        targets.push("windows".to_string());
    }
    if spec.targets.linux {
        targets.push("linux".to_string());
    }
    if spec.targets.macos {
        targets.push("macos".to_string());
    }

    Ctx {
        filekind_version: crate::VERSION.to_string(),
        banner_semi: super::banner(spec, ";"),
        banner_hash: super::banner(spec, "#"),
        banner_plain: super::banner(spec, "").trim_end().to_string(),

        name: spec.format.name.clone(),
        extension: spec.format.extension.clone(),
        dotted_ext: spec.dotted_extension(),
        description: spec.format.description.clone(),
        format_version: spec.format.version.clone().unwrap_or_else(|| "1.0".into()),
        container: spec.format.container.to_string(),
        slug: spec.slug(),

        mime: spec.association.mime.clone(),
        mime_icon_name: spec.mime_icon_name(),
        generic_icon,

        has_magic,
        magic_freedesktop: crate::magic::to_freedesktop(&magic),
        magic_libmagic: crate::magic::to_libmagic(&magic),
        magic_hex: crate::magic::to_hex_dump(&magic),
        magic_ascii: crate::magic::to_ascii_dump(&magic),

        has_handler,
        handler: handler.clone(),
        windows_command: escape::windows_command(
            &handler,
            &spec.handler_args_for(Platform::Windows),
        ),
        desktop_exec: escape::desktop_exec(&handler, &spec.handler_args_for(Platform::Linux)),

        progid: spec.progid(),
        has_perceived_type: spec.windows.perceived_type.is_some(),
        perceived_type: spec
            .windows
            .perceived_type
            .map(|p| p.as_str().to_string())
            .unwrap_or_default(),
        has_icon_path: spec.windows.icon_path.is_some(),
        icon_path: spec.windows.icon_path.clone().unwrap_or_default(),
        windows_default_icon,
        has_windows_default_icon,
        verbs: spec
            .windows
            .verbs
            .iter()
            .map(|(k, v)| Verb {
                name: k.clone(),
                command: v.clone(),
            })
            .collect(),

        desktop_id: spec.desktop_id(),
        desktop_file: format!("{}.desktop", spec.desktop_id()),
        mime_file: format!("{}-mime.xml", spec.slug()),
        categories: spec.linux.desktop_categories.clone(),
        has_generic_name: spec.linux.generic_name.is_some(),
        generic_name: spec.linux.generic_name.clone().unwrap_or_default(),
        no_display: spec.linux.no_display,

        uti: spec.uti(),
        uti_conforms_to: spec.uti_conforms_to(),
        bundle_id: spec.macos.bundle_id.clone().unwrap_or_else(|| spec.uti()),
        app_name: spec.format.name.clone(),

        scope: match opts.scope {
            Scope::User => "user".into(),
            Scope::System => "system".into(),
        },
        is_system: opts.scope.is_system(),
        hive: opts.scope.hive().to_string(),
        hive_short: opts.scope.hive_short().to_string(),
        xdg_mode: opts.scope.xdg_mode().to_string(),
        data_dir_expr: match opts.scope {
            Scope::User => "${XDG_DATA_HOME:-$HOME/.local/share}".into(),
            Scope::System => "${PREFIX:-/usr}/share".into(),
        },

        has_icon: opts.icon.is_some(),
        hicolor_sizes: crate::icon::HICOLOR_SIZES.to_vec(),
        icon_warnings: opts
            .icon
            .as_ref()
            .map(|i| i.warnings.iter().map(|w| w.to_string()).collect())
            .unwrap_or_default(),

        targets,
    }
}
