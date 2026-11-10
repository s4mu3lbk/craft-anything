use craft_anything_setup::{Host, clients, config};

const CODEX: &str = "# Codex config\nmodel = \"x\" # chosen\nnote = \"\"\"\nfirst\nsecond\"\"\"\n\n[mcp_servers.other]\ncommand = \"o\" # theirs\n";

fn crlf(s: &str) -> String {
    s.replace('\n', "\r\n")
}

fn cli() -> [String; 2] {
    ["/b/craft-anything-cli".to_string(), "mcp".to_string()]
}

/// A TOML config saved with Windows line endings keeps them when setup adds our entry, and
/// removing the entry again gives the file back byte for byte, as the JSONC and YAML configs do.
#[test]
fn a_crlf_toml_config_stays_crlf() {
    let all = clients(&Host::with_home("/h"));
    for id in ["codex", "grok_build"] {
        let client = all.iter().find(|c| c.id == id).unwrap();
        let entry = client.desired_entry(&cli());
        let original = crlf(CODEX);
        let new = config::set_entry(client, &original, &entry).unwrap();
        assert_eq!(new.matches('\n').count(), new.matches("\r\n").count(), "{id}: {new:?}");
        assert_eq!(config::read_entry(client, &new).unwrap(), Some(entry.clone()), "{id}");
        assert_eq!(config::parse(client, &new).unwrap()["note"], "first\r\nsecond", "{id}");
        assert_eq!(config::remove_entry(client, &new).unwrap().as_deref(), Some(original.as_str()), "{id}");
    }
}

/// A file with Unix line endings is written as before.
#[test]
fn an_lf_toml_config_stays_lf() {
    let all = clients(&Host::with_home("/h"));
    let codex = all.iter().find(|c| c.id == "codex").unwrap();
    let new = config::set_entry(codex, CODEX, &codex.desired_entry(&cli())).unwrap();
    assert!(!new.contains('\r'), "{new:?}");
    assert_eq!(config::remove_entry(codex, &new).unwrap().as_deref(), Some(CODEX));
}

/// A file that mixes both endings is left as toml_edit writes it. A line break inside a multi-line
/// string is part of its value, so swapping endings there would change what the app reads.
#[test]
fn a_mixed_toml_config_keeps_its_values() {
    let all = clients(&Host::with_home("/h"));
    let codex = all.iter().find(|c| c.id == "codex").unwrap();
    let crlf_lines = "a = 1\r\nb = 2\r\nc = 3\r\ns = \"\"\"\nx\ny\"\"\"\r\n";
    let crlf_string = "model = \"x\"\ninstructions = \"\"\"\r\none\r\ntwo\r\nthree\r\nfour\r\nfive\r\n\"\"\"\nnote = \"\"\"\nq\nr\"\"\"\n";
    for original in [crlf_lines, crlf_string] {
        let before = config::parse(codex, original).unwrap();
        let new = config::set_entry(codex, original, &codex.desired_entry(&cli())).unwrap();
        let mut after = config::parse(codex, &new).unwrap();
        after.as_object_mut().unwrap().remove("mcp_servers");
        assert_eq!(after, before, "{new:?}");
        let removed = config::remove_entry(codex, &new).unwrap().unwrap();
        let mut back = config::parse(codex, &removed).unwrap();
        back.as_object_mut().unwrap().remove("mcp_servers");
        assert_eq!(back, before, "{removed:?}");
    }
}
