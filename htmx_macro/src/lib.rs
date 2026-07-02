use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Expr, ExprLit, Lit, MetaNameValue,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------
#[allow(dead_code)]
struct RouteArgs {
    path: String,
    template: Option<String>,
    pavex_args: TokenStream2,
}

impl Parse for RouteArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let pairs = Punctuated::<MetaNameValue, Comma>::parse_terminated(input)?;

        let mut path = None;
        let mut template = None;
        let mut pavex_pairs: Vec<MetaNameValue> = Vec::new();

        for pair in &pairs {
            let key = pair.path.get_ident().map(|i| i.to_string());
            match key.as_deref() {
                Some("template") => {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) = &pair.value
                    {
                        template = Some(s.value());
                    } else {
                        return Err(syn::Error::new_spanned(
                            &pair.value,
                            "template must be a string literal",
                        ));
                    }
                }
                Some("path") => {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) = &pair.value
                    {
                        path = Some(s.value());
                    }
                    pavex_pairs.push(pair.clone());
                }
                _ => pavex_pairs.push(pair.clone()),
            }
        }

        let path = path.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing required argument `path`",
            )
        })?;

        let pavex_args = if pavex_pairs.is_empty() {
            quote! {}
        } else {
            let mut ts = TokenStream2::new();
            for (i, pair) in pavex_pairs.iter().enumerate() {
                ts.extend(quote! { #pair });
                if i < pavex_pairs.len() - 1 {
                    ts.extend(quote! { , });
                }
            }
            ts
        };

        Ok(RouteArgs {
            path,
            template,
            pavex_args,
        })
    }
}

// ---------------------------------------------------------------------------
// Route registry
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Route {
    method: String,
    path: String,
}

fn load_route_registry() -> Vec<Route> {
    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    let sdk_path = std::path::Path::new(&manifest_dir).join("../server_sdk/src/lib.rs");
    let source = match std::fs::read_to_string(&sdk_path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    parse_routes_from_sdk(&source)
}

fn parse_routes_from_sdk(source: &str) -> Vec<Route> {
    let lines: Vec<&str> = source.lines().collect();
    let mut routes = Vec::new();
    let mut current_method: Option<String> = None;

    for i in 0..lines.len() {
        let trimmed = lines[i].trim();

        if let Some(method) = extract_method_arm(trimmed) {
            current_method = Some(method);
            continue;
        }

        if trimmed.starts_with(
            "let matched_route_template = pavex::request::path::MatchedPathPattern::new(",
        ) {
            if let Some(ref method) = current_method
                && let Some(next) = lines.get(i + 1) {
                    let path = next.trim().trim_end_matches(',').trim_matches('"');
                    if path != "*" {
                        routes.push(Route {
                            method: method.clone(),
                            path: path.to_string(),
                        });
                    }
                }
            current_method = None;
            continue;
        }

        if trimmed == "_ => {" {
            current_method = None;
        }
    }

    routes
}

fn extract_method_arm(line: &str) -> Option<String> {
    let prefix = "&pavex::http::Method::";
    let pos = line.find(prefix)?;
    let rest = &line[pos + prefix.len()..];
    let method = rest.split_whitespace().next()?.trim_end_matches("=>");
    if ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].contains(&method) {
        Some(method.to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Template tag stripping
// ---------------------------------------------------------------------------

fn strip_template_tags(html: &str) -> String {
    let mut out = Vec::with_capacity(html.len());
    let b = html.as_bytes();
    let len = b.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && b[i] == b'{' && matches!(b[i + 1], b'%' | b'{' | b'#') {
            let end_char = if b[i + 1] == b'{' { b'}' } else { b'%' };
            out.push(b' ');
            out.push(b' ');
            i += 2;
            while i + 1 < len {
                if b[i] == end_char && b[i + 1] == b'}' {
                    out.push(b' ');
                    out.push(b' ');
                    i += 2;
                    break;
                }
                out.push(if b[i] == b'\n' { b'\n' } else { b' ' });
                i += 1;
            }
        } else {
            out.push(b[i]);
            i += 1;
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| html.to_string())
}

// ---------------------------------------------------------------------------
// Known constants
// ---------------------------------------------------------------------------

const VALID_SWAP_STRATEGIES: &[&str] = &[
    "innerHTML",
    "outerHTML",
    "beforebegin",
    "afterbegin",
    "beforeend",
    "afterend",
    "delete",
    "none",
];

const VALID_SYNC_STRATEGIES: &[&str] = &[
    "abort",
    "drop",
    "replace",
    "queue first",
    "queue last",
    "queue all",
];

const KNOWN_EXTENSIONS: &[&str] = &[
    "ajax-header",
    "alpine-morph",
    "class-tools",
    "client-side-templates",
    "debug",
    "disable-element",
    "event-header",
    "head-support",
    "include-vals",
    "json-enc",
    "idiomorph",
    "loading-states",
    "method-override",
    "morphdom-swap",
    "multi-swap",
    "path-deps",
    "preload",
    "remove-me",
    "response-targets",
    "restored",
    "server-sent-events",
    "web-sockets",
    "path-params",
];

const KNOWN_HX_ATTRS: &[&str] = &[
    "hx-get",
    "hx-post",
    "hx-put",
    "hx-patch",
    "hx-delete",
    "hx-push-url",
    "hx-replace-url",
    "hx-boost",
    "hx-vals",
    "hx-headers",
    "hx-swap",
    "hx-encoding",
    "hx-history",
    "hx-params",
    "hx-ext",
    "hx-target",
    "hx-indicator",
    "hx-select",
    "hx-select-oob",
    "hx-swap-oob",
    "hx-trigger",
    "hx-sync",
    "hx-include",
    "hx-disabled-elt",
    "hx-confirm",
    "hx-prompt",
    "hx-preserve",
    "hx-disable",
    "hx-validate",
    "hx-history-elt",
    "hx-vars",
    "hx-request",
    "hx-disinherit",
    "hx-inherit",
];

const KNOWN_REQUEST_KEYS: &[&str] = &["timeout", "credentials", "noHeaders"];

const VALID_TRIGGER_MODIFIERS: &[&str] = &[
    "once",
    "changed",
    "delay",
    "throttle",
    "from",
    "target",
    "consume",
    "queue",
    "root",
    "threshold",
];

const VALID_TRIGGER_EVENTS: &[&str] = &[
    "click",
    "change",
    "submit",
    "keyup",
    "keydown",
    "keypress",
    "mouseenter",
    "mouseleave",
    "mouseover",
    "mouseout",
    "mousemove",
    "mousedown",
    "mouseup",
    "focus",
    "blur",
    "input",
    "load",
    "revealed",
    "intersect",
    "every",
    "sse",
];

// ---------------------------------------------------------------------------
// Attr extraction helpers
// ---------------------------------------------------------------------------

/// All attributes on a start_tag node as (name, value, line).
/// Value is None for boolean attributes (no `=`).
fn extract_attrs(node: tree_sitter::Node, source: &str) -> Vec<(String, Option<String>, usize)> {
    let mut result = Vec::new();
    for attr in node.children(&mut node.walk()) {
        if attr.kind() != "attribute" {
            continue;
        }
        let mut name_node = None;
        let mut val_node = None;
        for child in attr.children(&mut attr.walk()) {
            match child.kind() {
                "attribute_name" => name_node = Some(child),
                "quoted_attribute_value" => val_node = Some(child),
                _ => {}
            }
        }
        if let Some(n) = name_node {
            let name = source[n.byte_range()].to_string();
            let line = n.start_position().row + 1;
            let val = val_node.map(|v| {
                source[v.byte_range()]
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string()
            });
            result.push((name, val, line));
        }
    }
    result
}

fn get_tag_name<'a>(start_tag: tree_sitter::Node, source: &'a str) -> &'a str {
    for child in start_tag.children(&mut start_tag.walk()) {
        if child.kind() == "tag_name" {
            return &source[child.byte_range()];
        }
    }
    ""
}

/// Collect all id attribute values in a document.
fn collect_ids(root: tree_sitter::Node, source: &str) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    collect_ids_recursive(root, source, &mut ids);
    ids
}

fn collect_ids_recursive(
    node: tree_sitter::Node,
    source: &str,
    ids: &mut std::collections::HashSet<String>,
) {
    if node.kind() == "start_tag" {
        for (name, val, _) in extract_attrs(node, source) {
            if name == "id"
                && let Some(v) = val {
                    ids.insert(v);
                }
        }
    }
    for child in node.children(&mut node.walk()) {
        collect_ids_recursive(child, source, ids);
    }
}

/// Count how many elements in the tree carry a given attribute name.
fn count_attr_occurrences(root: tree_sitter::Node, source: &str, attr_name: &str) -> usize {
    let mut count = 0;
    count_attr_recursive(root, source, attr_name, &mut count);
    count
}

fn count_attr_recursive(node: tree_sitter::Node, source: &str, attr_name: &str, count: &mut usize) {
    if node.kind() == "start_tag" {
        for (name, _, _) in extract_attrs(node, source) {
            if name == attr_name {
                *count += 1;
            }
        }
    }
    for child in node.children(&mut node.walk()) {
        count_attr_recursive(child, source, attr_name, count);
    }
}

/// Returns true if a value looks like a template expression (was stripped to spaces).
fn is_dynamic(value: &str) -> bool {
    value.trim().is_empty()
}

// ---------------------------------------------------------------------------
// Validation entry point
// ---------------------------------------------------------------------------

fn validate_template(html_raw: &str, template_file: &str, routes: &[Route]) -> Vec<String> {
    let mut errors = Vec::new();
    let html = strip_template_tags(html_raw);

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_html::LANGUAGE.into())
        .expect("failed to load HTML grammar");

    let tree = parser.parse(&html, None).unwrap();
    let root = tree.root_node();
    let ids = collect_ids(root, &html);

    walk_node(root, &html, template_file, routes, &ids, &mut errors);

    // hx-history-elt must appear at most once per rendered page.
    let history_elt_count = count_attr_occurrences(root, &html, "hx-history-elt");
    if history_elt_count > 1 {
        errors.push(format!(
            "hx-history-elt: found {} elements with this attribute in `{}` — only one per page is valid",
            history_elt_count, template_file
        ));
    }

    errors
}

// ---------------------------------------------------------------------------
// Tree walker
// ---------------------------------------------------------------------------

fn walk_node(
    node: tree_sitter::Node,
    source: &str,
    template_file: &str,
    routes: &[Route],
    ids: &std::collections::HashSet<String>,
    errors: &mut Vec<String>,
) {
    if node.kind() == "start_tag" {
        validate_start_tag(node, source, template_file, routes, ids, errors);
    }

    if node.kind() == "element" && has_attr_value(node, source, "hx-boost", "true") {
        check_boosted_children(node, source, routes, errors, template_file);
    }

    for child in node.children(&mut node.walk()) {
        walk_node(child, source, template_file, routes, ids, errors);
    }
}

fn validate_start_tag(
    node: tree_sitter::Node,
    source: &str,
    template_file: &str,
    routes: &[Route],
    ids: &std::collections::HashSet<String>,
    errors: &mut Vec<String>,
) {
    let tag_name = get_tag_name(node, source);
    let attrs = extract_attrs(node, source);

    // Collect hx-* attrs for context string and conflict checks.
    let hx_attrs: Vec<(String, String, usize)> = attrs
        .iter()
        .filter(|(n, v, _)| n.starts_with("hx-") && v.is_some())
        .map(|(n, v, l)| (n.clone(), v.clone().unwrap(), *l))
        .collect();

    if hx_attrs.is_empty() {
        return;
    }

    let tag_ctx: String = hx_attrs
        .iter()
        .map(|(k, v, _)| format!("{}=\"{}\"", k, v))
        .collect::<Vec<_>>()
        .join(" ");

    // Check for multiple hx-method attrs on same element.
    let method_attrs: Vec<&str> = hx_attrs
        .iter()
        .map(|(n, _, _)| n.as_str())
        .filter(|n| {
            matches!(
                *n,
                "hx-get" | "hx-post" | "hx-put" | "hx-patch" | "hx-delete"
            )
        })
        .collect();
    if method_attrs.len() > 1 {
        errors.push(format!(
            "multiple hx method attributes on same element: {} — only one is allowed\n  --> {}:{} <{}>",
            method_attrs.join(", "),
            template_file,
            hx_attrs[0].2,
            tag_ctx
        ));
    }

    // Check hx-push-url + hx-replace-url conflict.
    let has_push = hx_attrs.iter().any(|(n, _, _)| n == "hx-push-url");
    let has_replace = hx_attrs.iter().any(|(n, _, _)| n == "hx-replace-url");
    if has_push && has_replace {
        errors.push(format!(
            "hx-push-url and hx-replace-url both present — only one is allowed\n  --> {}:{} <{}>",
            template_file, hx_attrs[0].2, tag_ctx
        ));
    }

    for (name, value, line) in &hx_attrs {
        let ctx = format!("{}:{} <{}>", template_file, line, tag_ctx);

        if is_dynamic(value) {
            continue;
        }

        match name.as_str() {
            // --- route checks ---
            "hx-get" | "hx-post" | "hx-put" | "hx-patch" | "hx-delete" => {
                let method = name[3..].to_uppercase();
                validate_route(value, &method, routes, errors, &ctx);
            }

            "hx-push-url" | "hx-replace-url"
                if value != "true" && value != "false" => {
                    validate_route_path_exists(value, routes, errors, name, &ctx);
                }

            // --- json checks ---
            "hx-vals" | "hx-headers"
                if !value.starts_with("js:") => {
                    validate_json(value, name, errors, &ctx);
                }

            "hx-request" => {
                validate_hx_request(value, errors, &ctx);
            }

            // --- enum checks ---
            "hx-swap" => validate_hx_swap(value, errors, &ctx),

            "hx-encoding" => {
                if value != "multipart/form-data" {
                    errors.push(format!(
                        "hx-encoding: invalid value `{}` — only `multipart/form-data` is valid\n  --> {}",
                        value, ctx
                    ));
                }
                if tag_name != "form" {
                    errors.push(format!(
                        "hx-encoding: used on `<{}>` — only meaningful on `<form>`\n  --> {}",
                        tag_name, ctx
                    ));
                }
            }

            "hx-history"
                if value != "false" => {
                    errors.push(format!(
                        "hx-history: invalid value `{}` — only `false` is valid\n  --> {}",
                        value, ctx
                    ));
                }

            "hx-params" => {
                if tag_name != "form"
                    && tag_name != "input"
                    && tag_name != "select"
                    && tag_name != "textarea"
                    && tag_name != "button"
                {
                    errors.push(format!(
                        "hx-params: used on `<{}>` — only meaningful on form elements\n  --> {}",
                        tag_name, ctx
                    ));
                }
                validate_hx_params(value, errors, &ctx, template_file);
            }

            "hx-ext" => validate_hx_ext(value, errors, &ctx, template_file),

            "hx-sync" => validate_hx_sync(value, errors, &ctx),

            "hx-trigger" => validate_hx_trigger(value, errors, &ctx),

            "hx-disinherit" | "hx-inherit" => {
                validate_hx_attr_list(name, value, errors, &ctx);
            }

            "hx-swap-oob" => {
                let strategy = value.split(':').next().unwrap_or("").trim();
                if strategy != "true" && !VALID_SWAP_STRATEGIES.contains(&strategy) {
                    errors.push(format!(
                        "hx-swap-oob: invalid strategy `{}` — must be `true` or one of: {}\n  --> {}",
                        strategy,
                        VALID_SWAP_STRATEGIES.join(", "),
                        ctx
                    ));
                }
            }

            // --- id checks ---
            "hx-target" | "hx-indicator" => {
                validate_id_ref(name, value, ids, template_file, &ctx);
            }

            "hx-select"
                if let Some(id) = value.strip_prefix('#')
                    && !ids.contains(id) => {
                        errors.push(format!(
                            "{}: id `#{}` not found in template `{}`\n  --> {}",
                            name, id, template_file, ctx
                        ));
                    }

            "hx-select-oob" => {
                // Comma-separated list of "#id", "#id:strategy", or "strategy:#id".
                for part in value.split(',') {
                    let part = part.trim();
                    let colon = part.find(':');
                    let (selector, strategy_opt) = match colon {
                        None => (part, None),
                        Some(pos) => {
                            let left = part[..pos].trim();
                            let right = part[pos + 1..].trim();
                            // Whichever side starts with '#' is the selector.
                            if left.starts_with('#') {
                                (left, Some(right))
                            } else {
                                (right, Some(left))
                            }
                        }
                    };
                    if let Some(strategy) = strategy_opt
                        && !VALID_SWAP_STRATEGIES.contains(&strategy) {
                            errors.push(format!(
                                "hx-select-oob: invalid swap strategy `{}` — must be one of: {}\n  --> {}",
                                strategy,
                                VALID_SWAP_STRATEGIES.join(", "),
                                ctx
                            ));
                        }
                    if let Some(id) = selector.strip_prefix('#')
                        && !ids.contains(id) {
                            errors.push(format!(
                                "hx-select-oob: id `#{}` not found in template `{}`\n  --> {}",
                                id, template_file, ctx
                            ));
                        }
                }
            }

            // --- deprecation ---
            "hx-vars" => {
                errors.push(format!(
                    "hx-vars is deprecated — use `hx-vals` instead\n  --> {}",
                    ctx
                ));
            }

            // --- boolean attrs that require an id ---
            "hx-preserve" => {
                let has_id = attrs.iter().any(|(n, _, _)| n == "id");
                if !has_id {
                    errors.push(format!(
                        "hx-preserve: element must have an `id` attribute\n  --> {}",
                        ctx
                    ));
                }
            }

            // --- validate ---
            "hx-validate" => {
                if tag_name != "form" {
                    errors.push(format!(
                        "hx-validate: used on `<{}>` — only valid on `<form>`\n  --> {}",
                        tag_name, ctx
                    ));
                }
                if value != "true" {
                    errors.push(format!(
                        "hx-validate: invalid value `{}` — only `true` is valid\n  --> {}",
                        value, ctx
                    ));
                }
            }

            // --- boost misuse ---
            "hx-boost"
                if value == "true"
                    && tag_name != "a"
                    && tag_name != "form"
                    && tag_name != "div"
                    && tag_name != "section"
                    && tag_name != "nav"
                    && tag_name != "main"
                    && tag_name != "body"
                => {
                    errors.push(format!(
                        "hx-boost: used on `<{}>` — only meaningful on containers or `<a>`/`<form>`\n  --> {}",
                        tag_name, ctx
                    ));
                }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Individual validators
// ---------------------------------------------------------------------------

/// Strip query string and fragment from a path before route lookup.
///
/// `/items?page=2`  → `/items`
/// `/items#anchor`  → `/items`
/// `/items?q=1#top` → `/items`
///
/// Template expressions that partially survive (e.g. `/items?page=      `)
/// are handled because the `{` dynamic-path check runs after normalization.
fn normalize_path(path: &str) -> &str {
    let p = path.split('?').next().unwrap_or(path);
    p.split('#').next().unwrap_or(p)
}

fn validate_route(path: &str, method: &str, routes: &[Route], errors: &mut Vec<String>, ctx: &str) {
    let path = normalize_path(path);
    if path.contains('{') {
        return;
    }
    let path_exists = routes.iter().any(|r| r.path == path);
    if !path_exists {
        errors.push(format!(
            "hx-{}: route `{}` not found in route registry\n  --> {}\n      registered routes: [{}]",
            method.to_lowercase(),
            path,
            ctx,
            routes
                .iter()
                .map(|r| format!("{} {}", r.method, r.path))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        return;
    }
    let method_matches = routes.iter().any(|r| r.path == path && r.method == method);
    if !method_matches {
        let allowed: Vec<_> = routes
            .iter()
            .filter(|r| r.path == path)
            .map(|r| r.method.as_str())
            .collect();
        errors.push(format!(
            "hx-{}: route `{}` exists but method {} is not registered (allowed: {})\n  --> {}",
            method.to_lowercase(),
            path,
            method,
            allowed.join(", "),
            ctx
        ));
    }
}

fn validate_route_path_exists(
    path: &str,
    routes: &[Route],
    errors: &mut Vec<String>,
    attr: &str,
    ctx: &str,
) {
    let path = normalize_path(path);
    if path.contains('{') {
        return;
    }
    if !routes.iter().any(|r| r.path == path) {
        errors.push(format!(
            "{}: path `{}` not found in route registry\n  --> {}",
            attr, path, ctx
        ));
    }
}

fn validate_json(value: &str, attr: &str, errors: &mut Vec<String>, ctx: &str) {
    if serde_json::from_str::<serde_json::Value>(value).is_err() {
        errors.push(format!("{}: invalid JSON `{}`\n  --> {}", attr, value, ctx));
    }
}

fn validate_hx_request(value: &str, errors: &mut Vec<String>, ctx: &str) {
    match serde_json::from_str::<serde_json::Value>(value) {
        Err(_) => errors.push(format!(
            "hx-request: invalid JSON `{}`\n  --> {}",
            value, ctx
        )),
        Ok(v) => {
            if let Some(obj) = v.as_object() {
                for key in obj.keys() {
                    if !KNOWN_REQUEST_KEYS.contains(&key.as_str()) {
                        errors.push(format!(
                            "hx-request: unknown key `{}` — valid keys: {}\n  --> {}",
                            key,
                            KNOWN_REQUEST_KEYS.join(", "),
                            ctx
                        ));
                    }
                }
            }
        }
    }
}

fn validate_hx_swap(value: &str, errors: &mut Vec<String>, ctx: &str) {
    let strategy = value.split_whitespace().next().unwrap_or("");
    if !VALID_SWAP_STRATEGIES.contains(&strategy) {
        errors.push(format!(
            "hx-swap: invalid strategy `{}` — must be one of: {}\n  --> {}",
            strategy,
            VALID_SWAP_STRATEGIES.join(", "),
            ctx
        ));
    }
}

fn validate_hx_params(value: &str, errors: &mut Vec<String>, ctx: &str, template_file: &str) {
    if value == "*" || value == "none" {
        return;
    }
    // "not param1, param2" — exclusion list; strip the prefix then validate names.
    let names = value.strip_prefix("not ").unwrap_or(value);
    for part in names.split(',') {
        let part = part.trim();
        if part.is_empty()
            || !part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            errors.push(format!(
                "hx-params: invalid value `{}` — must be `*`, `none`, `not p1,p2`, or comma-separated param names\n  --> {}\n      in template `{}`",
                value, ctx, template_file
            ));
            return;
        }
    }
}

fn validate_hx_ext(value: &str, errors: &mut Vec<String>, ctx: &str, template_file: &str) {
    for ext in value.split(',') {
        let ext = ext.trim();
        // "ignore:ext-name" removes an inherited extension — strip prefix, validate name.
        let ext_name = ext.strip_prefix("ignore:").unwrap_or(ext);
        if !KNOWN_EXTENSIONS.contains(&ext_name) {
            errors.push(format!(
                "hx-ext: unknown extension `{}` — if custom, register it via hx_ext_register!(\"{}\")\n  --> {}\n      in template `{}`",
                ext_name, ext_name, ctx, template_file
            ));
        }
    }
}

fn validate_hx_sync(value: &str, errors: &mut Vec<String>, ctx: &str) {
    // Format: "selector:strategy" — strategy is optional (defaults to drop).
    let mut parts = value.splitn(2, ':');
    let _selector = parts.next(); // selector is opaque CSS; can't validate statically
    let strategy = match parts.next() {
        Some(s) => s.trim(),
        None => return, // no strategy = valid
    };
    // VALID_SYNC_STRATEGIES contains "queue first", "queue last", "queue all"
    // as full strings; splitn(2) gives us the whole strategy token intact.
    if !VALID_SYNC_STRATEGIES.contains(&strategy) {
        errors.push(format!(
            "hx-sync: invalid strategy `{}` — must be one of: {}\n  --> {}",
            strategy,
            VALID_SYNC_STRATEGIES.join(", "),
            ctx
        ));
    }
}

fn validate_hx_trigger(value: &str, errors: &mut Vec<String>, ctx: &str) {
    for trigger in value.split(',') {
        let trigger = trigger.trim();
        if trigger.is_empty() {
            continue;
        }
        let event = trigger.split_whitespace().next().unwrap_or("");
        // Strip filter expression: "click[ctrlKey]" → "click".
        let event_name = event.split('[').next().unwrap_or(event);

        if event_name == "every" {
            let rest: Vec<&str> = trigger.split_whitespace().collect();
            if rest.len() < 2 {
                // Structural error: "every" with no duration is always broken.
                errors.push(format!(
                    "hx-trigger: `every` requires a duration e.g. `every 2s`\n  --> {}",
                    ctx
                ));
            }
            continue;
        }

        if event_name == "sse" {
            continue;
        }

        if !VALID_TRIGGER_EVENTS.contains(&event_name) {
            // Warn, not error: custom and htmx-internal events are legitimate.
            eprintln!(
                "cargo:warning=hx-trigger: unknown event `{}` — may be a custom/htmx event or a typo. \
                 Known standard events: {}\n  --> {}",
                event_name,
                VALID_TRIGGER_EVENTS.join(", "),
                ctx
            );
        }

        // Modifier names are a closed spec — keep as errors.
        let tokens: Vec<&str> = trigger.split_whitespace().collect();
        for modifier in tokens.iter().skip(1) {
            let mod_name = modifier.split(':').next().unwrap_or(modifier);
            if !VALID_TRIGGER_MODIFIERS.contains(&mod_name) {
                errors.push(format!(
                    "hx-trigger: unknown modifier `{}` — valid modifiers: {}\n  --> {}",
                    mod_name,
                    VALID_TRIGGER_MODIFIERS.join(", "),
                    ctx
                ));
            }
        }
    }
}

fn validate_hx_attr_list(attr: &str, value: &str, errors: &mut Vec<String>, ctx: &str) {
    if value == "*" {
        return;
    }
    for name in value.split_whitespace() {
        if !KNOWN_HX_ATTRS.contains(&name) {
            errors.push(format!(
                "{}: `{}` is not a known htmx attribute\n  --> {}",
                attr, name, ctx
            ));
        }
    }
}

fn validate_id_ref(
    attr: &str,
    value: &str,
    ids: &std::collections::HashSet<String>,
    template_file: &str,
    ctx: &str,
) {
    // Keywords and relational selectors are opaque at compile time — skip.
    if value == "this"
        || value.starts_with("closest ")
        || value.starts_with("find ")
        || value.starts_with("next ")
        || value.starts_with("previous ")
        || value.starts_with("body")
    {
        return;
    }
    if let Some(id) = value.strip_prefix('#')
        && !ids.contains(id) {
            // Warn, not error: id may live in a base/parent template.
            eprintln!(
                "cargo:warning={}: id `#{}` not found in template `{}` (may be in base template)\n  --> {}",
                attr, id, template_file, ctx
            );
        }
}

// ---------------------------------------------------------------------------
// hx-boost child validation
// ---------------------------------------------------------------------------

fn has_attr_value(node: tree_sitter::Node, source: &str, attr_name: &str, attr_val: &str) -> bool {
    for child in node.children(&mut node.walk()) {
        if child.kind() == "start_tag" {
            for (name, val, _) in extract_attrs(child, source) {
                if name == attr_name && val.as_deref() == Some(attr_val) {
                    return true;
                }
            }
        }
    }
    false
}

fn check_boosted_children(
    node: tree_sitter::Node,
    source: &str,
    routes: &[Route],
    errors: &mut Vec<String>,
    template_file: &str,
) {
    for child in node.children(&mut node.walk()) {
        if child.kind() == "element" {
            for tag in child.children(&mut child.walk()) {
                if tag.kind() == "start_tag" {
                    let tag_name = get_tag_name(tag, source);
                    let attrs = extract_attrs(tag, source);

                    let mut href = None;
                    let mut action = None;
                    let mut method = "GET".to_string();
                    let mut line = 0usize;

                    for (name, val, l) in &attrs {
                        line = *l;
                        match name.as_str() {
                            "href" => href = val.clone(),
                            "action" => action = val.clone(),
                            "method" => method = val.as_deref().unwrap_or("get").to_uppercase(),
                            _ => {}
                        }
                    }

                    if tag_name == "a" {
                        if let Some(href) = href
                            && !is_dynamic(&href) {
                                let ctx = format!(
                                    "{}:{} <a href=\"{}\"> inside hx-boost",
                                    template_file, line, href
                                );
                                validate_route_path_exists(
                                    &href,
                                    routes,
                                    errors,
                                    "hx-boost href",
                                    &ctx,
                                );
                            }
                    } else if tag_name == "form"
                        && let Some(action) = action
                            && !is_dynamic(&action) {
                                let ctx = format!(
                                    "{}:{} <form action=\"{}\" method=\"{}\"> inside hx-boost",
                                    template_file, line, action, method
                                );
                                validate_route(&action, &method, routes, errors, &ctx);
                            }
                }
            }
            check_boosted_children(child, source, routes, errors, template_file);
        }
    }
}

// ---------------------------------------------------------------------------
// Core macro expansion
// ---------------------------------------------------------------------------

fn expand_hx_route(method: &str, args: TokenStream, input: TokenStream) -> TokenStream {
    let route_args = syn::parse_macro_input!(args as RouteArgs);
    let item = syn::parse_macro_input!(input as syn::ItemFn);
    let pavex_args = &route_args.pavex_args;
    let mut errors: Vec<String> = Vec::new();

    if let Some(ref template) = route_args.template {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let template_path = std::path::Path::new(&manifest_dir)
            .join("templates")
            .join(template);

        match std::fs::read_to_string(&template_path) {
            Err(e) => errors.push(format!("cannot read template `{}`: {}", template, e)),
            Ok(html_raw) => {
                let routes = load_route_registry();
                if !routes.is_empty() {
                    errors.append(&mut validate_template(&html_raw, template, &routes));
                }
            }
        }
    }

    if !errors.is_empty() {
        let error_tokens: TokenStream2 = errors
            .iter()
            .map(|e| quote! { compile_error!(#e); })
            .collect();
        return error_tokens.into();
    }

    let pavex_attr = match method {
        "GET" => quote! { #[pavex::get(#pavex_args)] },
        "POST" => quote! { #[pavex::post(#pavex_args)] },
        "PUT" => quote! { #[pavex::put(#pavex_args)] },
        "PATCH" => quote! { #[pavex::patch(#pavex_args)] },
        "DELETE" => quote! { #[pavex::delete(#pavex_args)] },
        _ => unreachable!(),
    };

    quote! { #pavex_attr #item }.into()
}

// ---------------------------------------------------------------------------
// Public proc-macro attributes
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn hx_get(args: TokenStream, input: TokenStream) -> TokenStream {
    expand_hx_route("GET", args, input)
}

#[proc_macro_attribute]
pub fn hx_post(args: TokenStream, input: TokenStream) -> TokenStream {
    expand_hx_route("POST", args, input)
}

#[proc_macro_attribute]
pub fn hx_put(args: TokenStream, input: TokenStream) -> TokenStream {
    expand_hx_route("PUT", args, input)
}

#[proc_macro_attribute]
pub fn hx_patch(args: TokenStream, input: TokenStream) -> TokenStream {
    expand_hx_route("PATCH", args, input)
}

#[proc_macro_attribute]
pub fn hx_delete(args: TokenStream, input: TokenStream) -> TokenStream {
    expand_hx_route("DELETE", args, input)
}
