use super::*;
use std::fs;
use tempfile::TempDir;

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, contents).expect("write config file");
}

#[test]
fn defaults_apply_when_no_config_files_exist() {
    let temp = TempDir::new().expect("temp dir");
    let loader = ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(temp.path().join("xdg"));

    let outcome = loader.load_from(temp.path());

    assert_eq!(outcome.config, Config::default());
    assert!(outcome.diagnostics.is_empty());
    assert!(outcome.project_path.is_none());
    assert!(outcome.user_path.is_none());
}

#[test]
fn tui_detail_show_edge_meta_loads_from_config() {
    // `[tui.detail].show_edge_meta` flips the runtime
    // default for the explorer's link-row meta visibility.
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.detail]\nshow_edge_meta = true\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty());
    assert!(outcome.config.tui.detail.show_edge_meta);
}

#[test]
fn tui_detail_show_edge_meta_defaults_to_false_when_absent() {
    let temp = TempDir::new().expect("temp dir");
    let loader = ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(temp.path().join("xdg"));
    let outcome = loader.load_from(temp.path());

    assert!(
        !outcome.config.tui.detail.show_edge_meta,
        "CSP-328: edge meta should default to hidden",
    );
}

#[test]
fn tui_narrow_layout_threshold_defaults_to_canonical_constant() {
    let temp = TempDir::new().expect("temp dir");
    let loader = ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(temp.path().join("xdg"));
    let outcome = loader.load_from(temp.path());

    assert!(outcome.diagnostics.is_empty());
    assert_eq!(
        outcome.config.tui.narrow_layout_threshold,
        crate::config::DEFAULT_NARROW_LAYOUT_THRESHOLD,
    );
}

#[test]
fn tui_narrow_layout_threshold_loads_from_config() {
    // `[tui] narrow_layout_threshold` overrides the
    // side-by-side → stacked reflow breakpoint.
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui]\nnarrow_layout_threshold = 80\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty());
    assert_eq!(outcome.config.tui.narrow_layout_threshold, 80);
}

#[test]
fn tui_narrow_layout_threshold_out_of_range_diagnoses_and_keeps_default() {
    // A value that can't fit in the u16 layout width (or is < 1)
    // produces a targeted diagnostic and leaves the default in place
    // rather than failing the whole file parse.
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui]\nnarrow_layout_threshold = 0\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(
        outcome.config.tui.narrow_layout_threshold,
        crate::config::DEFAULT_NARROW_LAYOUT_THRESHOLD,
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("narrow_layout_threshold")),
        "expected a narrow_layout_threshold diagnostic: {:?}",
        outcome.diagnostics,
    );
}

#[test]
fn project_config_parses_empty_table_subsections() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[table.sessions]\n[table.mux]\n[table.union]\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty());
    assert_eq!(
        outcome.project_path.as_deref(),
        Some(project.join(PROJECT_CONFIG_FILENAME).as_path())
    );
}

#[test]
fn project_config_search_walks_up_to_home_boundary() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    let nested = project.join("nested").join("deep");
    fs::create_dir_all(&nested).expect("create nested");
    write_file(&project.join(PROJECT_CONFIG_FILENAME), "[table.sessions]\n");

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&nested);

    assert!(outcome.diagnostics.is_empty());
    assert_eq!(
        outcome.project_path.as_deref(),
        Some(project.join(PROJECT_CONFIG_FILENAME).as_path())
    );
}

#[test]
fn project_config_search_stops_at_home() {
    let temp = TempDir::new().expect("temp dir");
    let home = temp.path().join("home");
    fs::create_dir_all(&home).expect("create home");
    // Place a file outside $HOME that would match if the walk
    // didn't stop. The loader must not pick it up.
    write_file(
        &temp.path().join(PROJECT_CONFIG_FILENAME),
        "[table.sessions]\n",
    );

    let loader = ConfigLoader::new().with_home(&home);
    let outcome = loader.load_from(&home);

    assert!(outcome.project_path.is_none());
}

#[test]
fn server_intervals_default_when_absent() {
    // No `[server]` block at all → every class keeps its ADR
    // 0038 default. The warm-start TTL gate falls back to
    // these numbers on a fresh install.
    let temp = TempDir::new().expect("temp dir");
    let loader = ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(temp.path().join("xdg"));
    let outcome = loader.load_from(temp.path());
    let intervals = outcome.config.server.intervals;
    assert_eq!(intervals.harness, Duration::from_secs(5));
    assert_eq!(intervals.mux, Duration::from_secs(5));
    assert_eq!(intervals.git, Duration::from_secs(30));
    assert_eq!(intervals.forge, Duration::from_secs(300));
}

#[test]
fn server_intervals_parse_each_supported_unit() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[server.intervals]\n\
             harness = \"500ms\"\n\
             mux     = \"10s\"\n\
             git     = \"2m\"\n\
             forge   = \"1h\"\n",
    );
    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);
    assert!(
        outcome.diagnostics.is_empty(),
        "expected no diagnostics; got {:?}",
        outcome.diagnostics
    );
    let intervals = outcome.config.server.intervals;
    assert_eq!(intervals.harness, Duration::from_millis(500));
    assert_eq!(intervals.mux, Duration::from_secs(10));
    assert_eq!(intervals.git, Duration::from_secs(120));
    assert_eq!(intervals.forge, Duration::from_secs(3600));
}

#[test]
fn server_intervals_unset_field_keeps_default() {
    // Partial override: only `forge` is bumped; the other
    // classes keep their defaults rather than collapsing to
    // zero. Per-field independence is the ADR 0079 contract.
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[server.intervals]\nforge = \"10m\"\n",
    );
    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);
    assert!(outcome.diagnostics.is_empty());
    let intervals = outcome.config.server.intervals;
    assert_eq!(intervals.harness, Duration::from_secs(5));
    assert_eq!(intervals.mux, Duration::from_secs(5));
    assert_eq!(intervals.git, Duration::from_secs(30));
    assert_eq!(intervals.forge, Duration::from_secs(600));
}

#[test]
fn server_intervals_malformed_value_diagnoses_and_keeps_default() {
    // A single malformed value emits a diagnostic and falls
    // back to the default for that field; the rest of the
    // table still merges. Matches the broader best-effort
    // merge contract elsewhere in this loader.
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[server.intervals]\nharness = \"5q\"\nmux = \"7s\"\n",
    );
    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);
    assert_eq!(
        outcome.diagnostics.len(),
        1,
        "exactly one diagnostic for the malformed value"
    );
    assert!(
        outcome.diagnostics[0].message.contains("harness"),
        "diagnostic should name the offending field; got {}",
        outcome.diagnostics[0].message
    );
    // Field falls back to default; other field still merged.
    assert_eq!(
        outcome.config.server.intervals.harness,
        Duration::from_secs(5)
    );
    assert_eq!(outcome.config.server.intervals.mux, Duration::from_secs(7));
}

#[test]
fn legacy_session_section_emits_diagnostic_but_does_not_abort() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[session]\nprojection = \"mux\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0].message.contains("session"),
        "diagnostic should mention the legacy section: {:?}",
        outcome.diagnostics[0].message,
    );
    assert!(outcome.diagnostics[0].message.contains("table"),);
    // Defaults still apply; the run is not aborted.
    assert_eq!(outcome.config, Config::default());
}

#[test]
fn malformed_toml_yields_diagnostic_and_default() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "this isn't toml = =",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.config, Config::default());
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.starts_with("malformed TOML"))
    );
}

#[test]
fn unknown_keys_are_silently_ignored() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[table.sessions]\nfuture_key = 1\n\n[unknown]\nx = \"y\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty());
}

#[test]
fn projection_round_trips_through_parse_and_as_str() {
    for variant in [
        Projection::Agent,
        Projection::Mux,
        Projection::Union,
        Projection::Pr,
        Projection::Fork,
    ] {
        assert_eq!(Projection::parse(variant.as_str()).unwrap(), variant);
    }
    // The `sessions` alias resolves to the same projection as the
    // historical `agent` token; both back the same row-type.
    assert_eq!(Projection::parse("sessions").unwrap(), Projection::Agent);
    // `pr` (singular) and `prs` (plural) both reach the Pr variant.
    assert_eq!(Projection::parse("pr").unwrap(), Projection::Pr);
    // Same alias pattern for forks.
    assert_eq!(Projection::parse("fork").unwrap(), Projection::Fork);
    assert!(Projection::parse("garbage").is_err());
}

#[test]
fn xdg_config_home_overrides_home_fallback() {
    let temp = TempDir::new().expect("temp dir");
    let xdg = temp.path().join("xdg");
    write_file(&xdg.join(USER_CONFIG_RELATIVE), "[table.sessions]\n");
    // Also place a colliding file under $HOME/.config that
    // should be ignored when XDG_CONFIG_HOME is set.
    write_file(
        &temp.path().join(".config").join(USER_CONFIG_RELATIVE),
        "[session]\nprojection = \"mux\"\n",
    );

    let loader = ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(&xdg);
    let outcome = loader.load_from(temp.path());

    // No diagnostic because the legacy file under $HOME/.config
    // was never opened (XDG took priority).
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(outcome.user_path.unwrap(), xdg.join(USER_CONFIG_RELATIVE));
}

#[test]
fn tui_scan_roots_parse_and_expand_home() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui]\nscan_roots = [\"~\", \"~/src/oss\", \"/abs/path\"]\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty());
    assert_eq!(
        outcome.config.tui.scan_roots,
        vec![
            temp.path().to_path_buf(),
            temp.path().join("src/oss"),
            PathBuf::from("/abs/path"),
        ]
    );
}

#[test]
fn tui_scan_roots_default_empty() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(&project.join(PROJECT_CONFIG_FILENAME), "[table.sessions]\n");

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.config.tui.scan_roots.is_empty());
}

#[test]
fn tui_views_grouping_parses_per_view() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.views.sessions]\ngrouping = \"repo\"\n\
             [tui.views.mux]\ngrouping = \"host\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(
        outcome.config.tui.views.sessions.grouping,
        Some(Grouping::Sessions(crate::tui::SessionsGrouping::Repo))
    );
    assert_eq!(
        outcome.config.tui.views.mux.grouping,
        Some(Grouping::Mux(crate::tui::MuxGrouping::Host))
    );
}

#[test]
fn tui_views_grouping_accepts_none_sessions_grouping() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.views.sessions]\ngrouping = \"none\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(
        outcome.config.tui.views.sessions.grouping,
        Some(Grouping::Sessions(crate::tui::SessionsGrouping::None))
    );
}

#[test]
fn tui_views_grouping_rejects_value_from_other_view() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.views.sessions]\ngrouping = \"host\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    // `host` is a mux grouping, not a sessions one.
    assert!(outcome.config.tui.views.sessions.grouping.is_none());
    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0]
            .message
            .contains("invalid `[tui.views.sessions].grouping`")
    );
}

#[test]
fn tui_views_filters_inline_single_entry() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.views.sessions]\n\
             filters = { harness = [\"claude-code\", \"codex\"], max_age = \"7d\", \
                         mux_state = [\"unmuxed\"] }\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let filter = &outcome.config.tui.views.sessions.filter;
    assert_eq!(
        filter.harness.as_ref().map(|h| h.values().to_vec()),
        Some(vec!["claude-code".to_string(), "codex".to_string()])
    );
    assert_eq!(
        filter.max_age,
        Some(std::time::Duration::from_secs(7 * 24 * 60 * 60))
    );
    assert_eq!(
        filter.mux_state.as_ref().map(|m| m.values().to_vec()),
        Some(vec![MuxStateKey::Unmuxed])
    );
}

#[test]
fn tui_views_filters_array_of_tables_unions_per_dimension() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[[tui.views.sessions.filters]]\nharness = [\"claude-code\"]\nmax_age = \"24h\"\n\
             [[tui.views.sessions.filters]]\nharness = [\"codex\"]\nmax_age = \"7d\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let filter = &outcome.config.tui.views.sessions.filter;
    // Harness sets union: both claude and codex.
    assert_eq!(
        filter.harness.as_ref().map(|h| h.values().to_vec()),
        Some(vec!["claude-code".to_string(), "codex".to_string()])
    );
    // Max-age takes the widest window (7d).
    assert_eq!(
        filter.max_age,
        Some(std::time::Duration::from_secs(7 * 24 * 60 * 60))
    );
}

#[test]
fn tui_views_filter_invalid_mux_state_emits_diagnostic() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.views.sessions]\n\
             filters = { mux_state = [\"unmuxed\", \"frobnicated\"] }\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0]
            .message
            .contains("invalid `[tui.views.sessions.filters].mux_state` value `frobnicated`")
    );
    // The valid entry still lands.
    let filter = &outcome.config.tui.views.sessions.filter;
    assert_eq!(
        filter.mux_state.as_ref().map(|m| m.values().to_vec()),
        Some(vec![MuxStateKey::Unmuxed])
    );
}

#[test]
fn legacy_sessions_grouping_seeds_new_key_with_deprecation_diagnostic() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui]\nsessions_grouping = \"checkout\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0]
            .message
            .contains("`[tui].sessions_grouping` is deprecated")
    );
    assert_eq!(
        outcome.config.tui.views.sessions.grouping,
        Some(Grouping::Sessions(crate::tui::SessionsGrouping::Checkout))
    );
}

#[test]
fn new_views_grouping_wins_over_legacy_when_both_present() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui]\nsessions_grouping = \"checkout\"\n\
             [tui.views.sessions]\ngrouping = \"scan-root\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    // Deprecation diagnostic still emits.
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("deprecated"))
    );
    // But the new key wins.
    assert_eq!(
        outcome.config.tui.views.sessions.grouping,
        Some(Grouping::Sessions(crate::tui::SessionsGrouping::ScanRoot))
    );
}

#[test]
fn legacy_sessions_grouping_invalid_value_emits_error_diagnostic() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui]\nsessions_grouping = \"nope\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    // Invalid legacy value emits an error diagnostic and does
    // not seed the new key.
    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0]
            .message
            .contains("invalid `[tui].sessions_grouping`")
    );
    assert!(outcome.config.tui.views.sessions.grouping.is_none());
}

#[test]
fn parse_config_duration_handles_each_unit() {
    assert_eq!(parse_config_duration("30s"), Ok(30));
    assert_eq!(parse_config_duration("2m"), Ok(120));
    assert_eq!(parse_config_duration("1h"), Ok(3_600));
    assert_eq!(parse_config_duration("7d"), Ok(7 * 24 * 60 * 60));
    assert_eq!(parse_config_duration("2000ms"), Ok(2));
    assert_eq!(parse_config_duration("42"), Ok(42));
    assert!(parse_config_duration("").is_err());
    assert!(parse_config_duration("nope").is_err());
    assert!(parse_config_duration("12years").is_err());
}

#[test]
fn tui_theme_overrides_named_color_and_modifier() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme]\n\
             harness_claude = \"bright_red\"\n\
             mux_attached = \"#00ff88\"\n\
             selection_active = \"reversed,italic\"\n\
             recency_cold = \"dim,italic\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let theme = &outcome.config.tui.theme;
    assert_eq!(
        theme.harness_color("claude-code"),
        ratatui::style::Color::LightRed
    );
    assert_eq!(
        theme.mux_attached,
        ratatui::style::Color::Rgb(0x00, 0xff, 0x88)
    );
    assert!(
        theme
            .selection_active
            .contains(ratatui::style::Modifier::REVERSED)
    );
    assert!(
        theme
            .selection_active
            .contains(ratatui::style::Modifier::ITALIC)
    );
    assert!(
        theme
            .recency_cold
            .modifier
            .contains(ratatui::style::Modifier::DIM)
    );
}

#[test]
fn tui_theme_unspecified_keys_keep_defaults() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme]\nharness_codex = \"yellow\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    let theme = &outcome.config.tui.theme;
    let defaults = Theme::default();
    assert_eq!(theme.harness_color("codex"), ratatui::style::Color::Yellow);
    // Untouched fields stay at default values.
    assert_eq!(
        theme.harness_color("claude-code"),
        defaults.harness_color("claude-code")
    );
    assert_eq!(theme.mux_attached, defaults.mux_attached);
}

#[test]
fn tui_theme_unknown_key_emits_diagnostic_and_keeps_defaults() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme]\nnot_a_field = \"red\"\nharness_claude = \"green\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0]
            .message
            .contains("unknown `[tui.theme]` key `not_a_field`")
    );
    // Recognized neighbors still apply.
    assert_eq!(
        outcome.config.tui.theme.harness_color("claude-code"),
        ratatui::style::Color::Green
    );
}

#[test]
fn tui_theme_malformed_value_warns_and_falls_back() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme]\nharness_claude = \"#zzzzzz\"\nmux_attached = \"green\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0]
            .message
            .contains("`[tui.theme].harness_claude`")
    );
    let defaults = Theme::default();
    assert_eq!(
        outcome.config.tui.theme.harness_color("claude-code"),
        defaults.harness_color("claude-code"),
        "bad spec falls back to default"
    );
    // Sibling fields still parse.
    assert_eq!(
        outcome.config.tui.theme.mux_attached,
        ratatui::style::Color::Green
    );
}

#[test]
fn tui_theme_non_string_value_emits_diagnostic() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme]\nharness_claude = 42\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(outcome.diagnostics[0].message.contains("must be a string"));
}

#[test]
fn tui_theme_harness_table_overrides_per_key_colors() {
    // `[tui.theme.harness].<key> = "color"` sets
    // theme.harness_colors[<key>]. Adapter-registry-registered
    // keys are accepted; unknown keys emit a diagnostic
    // pointing at the registered set.
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme.harness]\nclaude-code = \"bright_blue\"\ncodex = \"magenta\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let theme = &outcome.config.tui.theme;
    assert_eq!(
        theme.harness_color("claude-code"),
        ratatui::style::Color::LightBlue
    );
    assert_eq!(theme.harness_color("codex"), ratatui::style::Color::Magenta);
    // Unlisted registered adapters keep their defaults.
    let defaults = Theme::default();
    assert_eq!(
        theme.harness_color("opencode"),
        defaults.harness_color("opencode")
    );
}

#[test]
fn tui_theme_harness_table_unknown_key_emits_diagnostic() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme.harness]\nnever-registered = \"red\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    let msg = &outcome.diagnostics[0].message;
    assert!(
        msg.contains("unknown `[tui.theme.harness]` key `never-registered`"),
        "{msg}"
    );
    assert!(msg.contains("registered harnesses:"), "{msg}");
}

#[test]
fn tui_theme_flat_harness_alias_still_works() {
    // The older flat keys stay as aliases per ADR 0031.
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme]\nharness_codex = \"magenta\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(
        outcome.config.tui.theme.harness_color("codex"),
        ratatui::style::Color::Magenta
    );
}

#[test]
fn tui_theme_icons_overrides_glyph_per_node_kind() {
    use crate::tui::icons::NodeKind;

    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme.icons]\nnode_fork = \"Y\"\nnode_repo = \"R\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert!(
        outcome.diagnostics.is_empty(),
        "no diagnostics expected, got {:?}",
        outcome.diagnostics
    );
    let icons = &outcome.config.tui.theme.icons;
    assert_eq!(icons.glyph(NodeKind::Fork).as_deref(), Some("Y"));
    assert_eq!(icons.glyph(NodeKind::Repo).as_deref(), Some("R"));
    assert!(
        icons.glyph(NodeKind::Workspace).is_none(),
        "untouched kinds fall through to the default slate"
    );
}

#[test]
fn tui_theme_icons_rejects_wide_glyph_with_diagnostic() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    // CJK ideograph is unambiguously East Asian Wide (2 cells).
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme.icons]\nnode_workspace = \"中\"\nnode_fork = \"Y\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0].message.contains("display width"),
        "expected width diagnostic, got: {}",
        outcome.diagnostics[0].message
    );
    // Sibling valid override still applies.
    assert_eq!(
        outcome
            .config
            .tui
            .theme
            .icons
            .glyph(crate::tui::icons::NodeKind::Fork)
            .as_deref(),
        Some("Y"),
    );
}

#[test]
fn tui_theme_icons_unknown_key_emits_diagnostic() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui.theme.icons]\nworkspace_glyph = \"▦\"\n",
    );

    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        outcome.diagnostics[0]
            .message
            .contains("unknown `[tui.theme.icons]` key `workspace_glyph`"),
        "got: {}",
        outcome.diagnostics[0].message,
    );
}

#[test]
fn expand_home_preserves_paths_without_tilde() {
    let home = PathBuf::from("/home/op");
    assert_eq!(
        expand_home("/etc/conspectus", Some(&home)),
        PathBuf::from("/etc/conspectus")
    );
    assert_eq!(expand_home("~", Some(&home)), home);
    assert_eq!(expand_home("~/src", Some(&home)), home.join("src"));
    // `~suffix` without `/` is not a recognized form; leave as-is
    // so we don't accidentally splice the username's home for
    // some other user.
    assert_eq!(expand_home("~root", Some(&home)), PathBuf::from("~root"));
    // Without a home directory we cannot expand; keep the raw
    // string rather than silently dropping the `~`.
    assert_eq!(expand_home("~/src", None), PathBuf::from("~/src"));
}

#[test]
fn worktree_backend_defaults_to_auto() {
    let temp = TempDir::new().expect("temp dir");
    let loader = ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(temp.path().join("xdg"));
    let outcome = loader.load_from(temp.path());
    assert_eq!(
        outcome.config.worktree.backend,
        crate::discovery::worktree::WorktreeBackendSelection::Auto,
    );
}

#[test]
fn worktree_backend_loads_from_config() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[worktree]\nbackend = \"worktrunk\"\n",
    );
    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(
        outcome.config.worktree.backend,
        crate::discovery::worktree::WorktreeBackendSelection::Worktrunk,
    );
}

#[test]
fn worktree_backend_invalid_value_diagnoses_and_keeps_default() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[worktree]\nbackend = \"svn\"\n",
    );
    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);
    assert_eq!(
        outcome.config.worktree.backend,
        crate::discovery::worktree::WorktreeBackendSelection::Auto,
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("[worktree] backend")),
        "expected a worktree backend diagnostic: {:?}",
        outcome.diagnostics,
    );
}

#[test]
fn worktree_teardown_defaults_to_live_and_three_seconds() {
    let temp = TempDir::new().expect("temp dir");
    let loader = ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(temp.path().join("xdg"));
    let outcome = loader.load_from(temp.path());
    assert_eq!(
        outcome.config.worktree.teardown_confirm,
        crate::config::TeardownConfirm::Live,
    );
    assert_eq!(
        outcome.config.worktree.teardown_grace,
        std::time::Duration::from_secs(3),
    );
}

#[test]
fn worktree_teardown_keys_load_from_config() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[worktree]\nteardown_confirm = \"never\"\nteardown_grace = \"500ms\"\n",
    );
    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(
        outcome.config.worktree.teardown_confirm,
        crate::config::TeardownConfirm::Never,
    );
    assert_eq!(
        outcome.config.worktree.teardown_grace,
        std::time::Duration::from_millis(500),
    );
}

#[test]
fn worktree_teardown_confirm_invalid_diagnoses_and_keeps_default() {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[worktree]\nteardown_confirm = \"sometimes\"\n",
    );
    let loader = ConfigLoader::new().with_home(temp.path());
    let outcome = loader.load_from(&project);
    assert_eq!(
        outcome.config.worktree.teardown_confirm,
        crate::config::TeardownConfirm::Live,
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("[worktree] teardown_confirm")),
        "expected a teardown_confirm diagnostic: {:?}",
        outcome.diagnostics,
    );
}

fn load_tui_config(body: &str) -> crate::config::LoadOutcome {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(&project.join(PROJECT_CONFIG_FILENAME), body);
    ConfigLoader::new()
        .with_home(temp.path())
        .load_from(&project)
}

#[test]
fn tui_default_view_is_unset_by_default() {
    let outcome = load_tui_config("[tui]\n");
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(outcome.config.tui.default_view, None);
}

#[test]
fn tui_default_view_loads_from_config() {
    let outcome = load_tui_config("[tui]\ndefault_view = \"Mux\"\n");
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(outcome.config.tui.default_view, Some(crate::tui::View::Mux));
}

#[test]
fn tui_default_view_unknown_value_diagnoses_and_stays_unset() {
    let outcome = load_tui_config("[tui]\ndefault_view = \"graph\"\n");
    assert_eq!(outcome.config.tui.default_view, None);
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("default_view")),
        "{:?}",
        outcome.diagnostics
    );
}

#[test]
fn tui_preview_wrap_defaults_to_smart_and_loads_from_config() {
    assert_eq!(
        load_tui_config("[tui]\n").config.tui.preview_wrap,
        crate::tui::PreviewWrap::Smart
    );
    let outcome = load_tui_config("[tui]\npreview_wrap = \"none\"\n");
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(
        outcome.config.tui.preview_wrap,
        crate::tui::PreviewWrap::None
    );
}

#[test]
fn tui_preview_wrap_unknown_value_diagnoses_and_keeps_the_default() {
    let outcome = load_tui_config("[tui]\npreview_wrap = \"fancy\"\n");
    assert_eq!(
        outcome.config.tui.preview_wrap,
        crate::tui::PreviewWrap::Smart
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.message.contains("preview_wrap")),
        "{:?}",
        outcome.diagnostics
    );
}

fn load_theme_badge_width(raw: &str) -> LoadOutcome {
    let temp = TempDir::new().expect("temp dir");
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        &format!("[tui.theme]\nbadge_width = {raw}\n"),
    );
    ConfigLoader::new()
        .with_home(temp.path())
        .load_from(&project)
}

#[test]
fn tui_theme_badge_width_overrides_the_default() {
    let outcome = load_theme_badge_width("12");

    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(outcome.config.tui.theme.badge_width, 12);
}

#[test]
fn tui_theme_badge_width_rejects_values_below_the_minimum_or_non_integers() {
    for raw in ["1", "-3", "\"wide\""] {
        let outcome = load_theme_badge_width(raw);

        assert_eq!(outcome.diagnostics.len(), 1, "{raw}");
        assert!(
            outcome.diagnostics[0].message.contains("badge_width"),
            "{raw}: {}",
            outcome.diagnostics[0].message
        );
        assert_eq!(
            outcome.config.tui.theme.badge_width,
            crate::tui::theme::DEFAULT_BADGE_WIDTH
        );
    }
}

#[test]
fn load_without_anchor_skips_the_project_walk_and_keeps_user_config() {
    // ADR 0111: a deleted launch directory leaves no anchor; user
    // config still applies and nothing errors.
    let temp = TempDir::new().expect("temp dir");
    let xdg = temp.path().join("xdg");
    fs::create_dir_all(xdg.join("conspectus")).expect("create xdg dir");
    write_file(
        &xdg.join("conspectus").join("config.toml"),
        "[tui]\nnarrow_layout_threshold = 90\n",
    );
    let project = temp.path().join("project");
    fs::create_dir(&project).expect("create project dir");
    write_file(
        &project.join(PROJECT_CONFIG_FILENAME),
        "[tui]\nnarrow_layout_threshold = 80\n",
    );
    let loader = ConfigLoader::new()
        .with_home(temp.path())
        .with_xdg_config_home(&xdg);

    let anchored = loader.load(Some(&project));
    assert_eq!(anchored.config.tui.narrow_layout_threshold, 80);

    let unanchored = loader.load(None);
    assert!(unanchored.diagnostics.is_empty());
    assert!(unanchored.project_path.is_none());
    assert_eq!(unanchored.config.tui.narrow_layout_threshold, 90);
}
