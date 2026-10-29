use std::collections::HashSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MANAGED_DIRECTIVES: [&str; 4] = ["HostName", "User", "Port", "IdentityFile"];
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub alias: String,
    pub group: Option<String>,
    pub aliases: Vec<String>,
    pub host_name: String,
    pub user: String,
    pub port: String,
    pub identity_file: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostDraft {
    pub aliases: String,
    pub host_name: String,
    pub user: String,
    pub port: String,
    pub identity_file: String,
    pub group: String,
}

#[derive(Debug)]
struct ValidatedDraft {
    aliases: Vec<String>,
    values: [String; 4],
    group: String,
}

#[derive(Debug, Clone, Copy)]
struct ConfigLine<'a> {
    full: &'a str,
    text: &'a str,
    start: usize,
}

pub fn load_hosts() -> Result<Vec<Host>, String> {
    let path = config_path()?;

    match fs::read_to_string(&path) {
        Ok(contents) => Ok(primary_hosts(parse_hosts(&contents))),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

fn primary_hosts(hosts: Vec<Host>) -> Vec<Host> {
    let mut primary = Vec::new();
    for host in hosts {
        if primary
            .last()
            .is_some_and(|previous: &Host| previous.aliases == host.aliases)
        {
            continue;
        }
        primary.push(host);
    }
    primary
}

pub fn save_host(original_alias: Option<&str>, draft: &HostDraft) -> Result<(), String> {
    let path = config_path()?;
    save_host_at(&path, original_alias, draft)
}

pub fn delete_host(alias: &str) -> Result<(), String> {
    let path = config_path()?;
    delete_host_at(&path, alias)
}

fn config_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .ok_or_else(|| "HOME is not set; cannot locate ~/.ssh/config".to_owned())?;
    Ok(PathBuf::from(home).join(".ssh").join("config"))
}

fn save_host_at(
    path: &Path,
    original_alias: Option<&str>,
    draft: &HostDraft,
) -> Result<(), String> {
    let draft = validate_draft(draft)?;
    let mut contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound && original_alias.is_none() => {
            String::new()
        }
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };

    match original_alias {
        Some(original_alias) => {
            contents = update_host_block(&contents, original_alias, &draft)?;
        }
        None => append_host_block(&mut contents, &draft),
    }

    atomic_write(path, &contents)
}

fn delete_host_at(path: &Path, alias: &str) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let updated = delete_host_block(&contents, alias)?;
    atomic_write(path, &updated)
}

fn validate_draft(draft: &HostDraft) -> Result<ValidatedDraft, String> {
    let mut aliases = Vec::new();
    let mut seen = HashSet::new();

    for alias in draft.aliases.split_whitespace() {
        if alias.starts_with('#') {
            return Err("host aliases cannot start with '#'".to_owned());
        }
        if alias.starts_with('!') || alias.contains(['*', '?']) {
            return Err(format!(
                "host alias '{alias}' must be a concrete name without negation or wildcards"
            ));
        }
        if alias.starts_with('-')
            || !alias.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
            })
        {
            return Err(format!(
                "host alias '{alias}' may contain only ASCII letters, digits, '.', '_' and '-'"
            ));
        }
        if seen.insert(alias.to_owned()) {
            aliases.push(alias.to_owned());
        }
    }

    if aliases.is_empty() {
        return Err("at least one host alias is required".to_owned());
    }

    let raw_values = [
        &draft.host_name,
        &draft.user,
        &draft.port,
        &draft.identity_file,
        &draft.group,
    ];
    let names = ["HostName", "User", "Port", "IdentityFile", "group"];
    for (value, name) in raw_values.into_iter().zip(names) {
        if value.contains(['\r', '\n']) {
            return Err(format!("{name} cannot contain newlines"));
        }
    }

    let port = draft.port.trim();
    if !port.is_empty() && port.parse::<u16>().ok().filter(|port| *port != 0).is_none() {
        return Err("Port must be empty or a number from 1 to 65535".to_owned());
    }

    Ok(ValidatedDraft {
        aliases,
        values: [
            draft.host_name.trim().to_owned(),
            draft.user.trim().to_owned(),
            port.to_owned(),
            draft.identity_file.trim().to_owned(),
        ],
        group: draft.group.trim().to_owned(),
    })
}

fn append_host_block(contents: &mut String, draft: &ValidatedDraft) {
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&canonical_group_line(&draft.group));
    contents.push_str(&format!("Host {}\n", draft.aliases.join(" ")));
    for (keyword, value) in MANAGED_DIRECTIVES.iter().zip(&draft.values) {
        if !value.is_empty() {
            contents.push_str(&format!("    {keyword} {value}\n"));
        }
    }
}

fn update_host_block(
    contents: &str,
    original_alias: &str,
    draft: &ValidatedDraft,
) -> Result<String, String> {
    let lines = config_lines(contents);
    let (host_index, block_end) = find_host_block(&lines, original_alias)
        .ok_or_else(|| format!("host '{original_alias}' was not found"))?;
    let host_line = lines[host_index];
    let existing_group = group_at_host(&lines, host_index);
    let owned_group = (0..host_index).rev().find(|index| {
        parse_group_directive(lines[*index].text).is_some()
            && !lines[*index + 1..host_index]
                .iter()
                .any(|line| is_directive(line.text, "Host"))
    });
    let replacement_start = owned_group.unwrap_or(host_index);
    let newline = preferred_newline(contents);

    let mut updated = String::with_capacity(contents.len() + 64);
    updated.push_str(&contents[..lines[replacement_start].start]);

    if owned_group.is_some() || draft.group != existing_group.as_deref().unwrap_or("") {
        let ending = owned_group.map_or(newline, |index| line_ending(lines[index].full));
        updated.push_str(&canonical_group_line_with_ending(
            &draft.group,
            ending,
            true,
        ));
    }
    if let Some(group_index) = owned_group {
        for line in &lines[group_index + 1..host_index] {
            updated.push_str(line.full);
        }
    }

    let mut present = [false; 4];
    for line in &lines[host_index + 1..block_end] {
        if let Some(index) = managed_directive_index(line.text) {
            present[index] = true;
        }
    }

    let has_following_lines = block_end > host_index + 1;
    let inserts_directive = (0..4).any(|index| !present[index] && !draft.values[index].is_empty());
    updated.push_str(&format!("Host {}", draft.aliases.join(" ")));
    if line_ending(host_line.full).is_empty() && (has_following_lines || inserts_directive) {
        updated.push_str(newline);
    } else {
        updated.push_str(line_ending(host_line.full));
    }

    for index in 0..4 {
        if !present[index] && !draft.values[index].is_empty() {
            updated.push_str(&format!(
                "    {} {}{}",
                MANAGED_DIRECTIVES[index], draft.values[index], newline
            ));
        }
    }

    let mut emitted = [false; 4];
    for line in &lines[host_index + 1..block_end] {
        if let Some(index) = managed_directive_index(line.text) {
            if emitted[index] {
                continue;
            }
            emitted[index] = true;
            if draft.values[index].is_empty() {
                continue;
            }
            updated.push_str(&format!(
                "    {} {}{}",
                MANAGED_DIRECTIVES[index],
                draft.values[index],
                line_ending(line.full)
            ));
        } else {
            updated.push_str(line.full);
        }
    }

    let group_changed = draft.group != existing_group.as_deref().unwrap_or("");
    let next_host_inherits_group = block_end < lines.len()
        && is_directive(lines[block_end].text, "Host")
        && !lines[host_index + 1..block_end]
            .iter()
            .any(|line| parse_group_directive(line.text).is_some())
        && group_at_host(&lines, block_end) == existing_group;
    if group_changed && next_host_inherits_group {
        updated.push_str(&canonical_group_line_with_ending(
            existing_group.as_deref().unwrap_or(""),
            newline,
            true,
        ));
    }

    let suffix_start = lines
        .get(block_end)
        .map_or(contents.len(), |line| line.start);
    updated.push_str(&contents[suffix_start..]);
    Ok(updated)
}

fn delete_host_block(contents: &str, alias: &str) -> Result<String, String> {
    let lines = config_lines(contents);
    let (host_index, block_end) =
        find_host_block(&lines, alias).ok_or_else(|| format!("host '{alias}' was not found"))?;
    // Keep a shared group directive when following hosts inherit it. If no
    // following host needs it, remove this block's owned directive as well.
    let has_following_host = lines[block_end..]
        .iter()
        .any(|line| is_directive(line.text, "Host"));
    let owned_group = (0..host_index).rev().find(|index| {
        parse_group_directive(lines[*index].text).is_some()
            && !lines[*index + 1..host_index]
                .iter()
                .any(|line| is_directive(line.text, "Host"))
    });
    let start_index = if has_following_host {
        host_index
    } else {
        owned_group.unwrap_or(host_index)
    };

    // Comments and whitespace immediately before the next block commonly document
    // that block, so leave them outside the deletion range.
    let mut deletion_end = block_end;
    if block_end < lines.len() {
        while deletion_end > host_index + 1 && is_blank_or_comment(lines[deletion_end - 1].text) {
            deletion_end -= 1;
        }
    }

    let mut updated = String::with_capacity(contents.len());
    updated.push_str(&contents[..lines[start_index].start]);
    let suffix_start = lines
        .get(deletion_end)
        .map_or(contents.len(), |line| line.start);
    updated.push_str(&contents[suffix_start..]);
    Ok(updated)
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let permissions = match fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(format!("failed to inspect {}: {error}", path.display())),
    };

    let (temporary_path, mut temporary_file) = create_temporary_file(parent)?;
    let result = (|| -> Result<(), String> {
        if let Some(permissions) = permissions {
            fs::set_permissions(&temporary_path, permissions).map_err(|error| {
                format!(
                    "failed to preserve permissions on {}: {error}",
                    temporary_path.display()
                )
            })?;
        }
        temporary_file
            .write_all(contents.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", temporary_path.display()))?;
        temporary_file
            .sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", temporary_path.display()))?;
        drop(temporary_file);
        fs::rename(&temporary_path, path).map_err(|error| {
            format!(
                "failed to replace {} with {}: {error}",
                path.display(),
                temporary_path.display()
            )
        })?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn create_temporary_file(parent: &Path) -> Result<(PathBuf, fs::File), String> {
    for _ in 0..100 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".config.ssh-tray-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to create temporary file in {}: {error}",
                    parent.display()
                ));
            }
        }
    }
    Err(format!(
        "failed to allocate a temporary file in {}",
        parent.display()
    ))
}

fn parse_hosts(contents: &str) -> Vec<Host> {
    let lines = config_lines(contents);
    let mut hosts = Vec::new();
    let mut seen = HashSet::new();
    let mut current_group = None;
    let mut index = 0;

    while index < lines.len() {
        if let Some(group) = parse_group_directive(lines[index].text) {
            current_group = group;
            index += 1;
            continue;
        }

        if !is_directive(lines[index].text, "Host") {
            index += 1;
            continue;
        }

        let block_end = host_block_end(&lines, index);
        let aliases = host_aliases(lines[index].text);
        let values = managed_values(&lines[index + 1..block_end]);
        let host_group = current_group.clone();

        for alias in &aliases {
            if seen.insert(alias.clone()) {
                hosts.push(Host {
                    alias: alias.clone(),
                    group: host_group.clone(),
                    aliases: aliases.clone(),
                    host_name: values[0].clone(),
                    user: values[1].clone(),
                    port: values[2].clone(),
                    identity_file: values[3].clone(),
                });
            }
        }

        for line in &lines[index + 1..block_end] {
            if let Some(group) = parse_group_directive(line.text) {
                current_group = group;
            }
        }
        index = block_end;
    }

    hosts
}

fn managed_values(lines: &[ConfigLine<'_>]) -> [String; 4] {
    let mut values: [String; 4] = std::array::from_fn(|_| String::new());
    let mut found = [false; 4];
    for line in lines {
        let Some(index) = managed_directive_index(line.text) else {
            continue;
        };
        if !found[index] {
            values[index] = directive(line.text)
                .map(|(_, value)| value.to_owned())
                .unwrap_or_default();
            found[index] = true;
        }
    }
    values
}

fn find_host_block(lines: &[ConfigLine<'_>], alias: &str) -> Option<(usize, usize)> {
    (0..lines.len())
        .filter(|index| is_directive(lines[*index].text, "Host"))
        .map(|index| (index, host_block_end(lines, index)))
        .find(|(index, _)| {
            host_aliases(lines[*index].text)
                .iter()
                .any(|item| item == alias)
        })
}

fn host_block_end(lines: &[ConfigLine<'_>], host_index: usize) -> usize {
    (host_index + 1..lines.len())
        .find(|index| {
            is_directive(lines[*index].text, "Host") || is_directive(lines[*index].text, "Match")
        })
        .unwrap_or(lines.len())
}

fn group_at_host(lines: &[ConfigLine<'_>], host_index: usize) -> Option<String> {
    let mut group = None;
    for line in &lines[..host_index] {
        if let Some(next_group) = parse_group_directive(line.text) {
            group = next_group;
        }
    }
    group
}

fn host_aliases(line: &str) -> Vec<String> {
    let Some((_, value)) = directive(line) else {
        return Vec::new();
    };
    let mut aliases = Vec::new();
    for alias in value.split_whitespace() {
        if alias.starts_with('#') {
            break;
        }
        if !alias.starts_with('!') && !alias.contains(['*', '?']) {
            aliases.push(alias.to_owned());
        }
    }
    aliases
}

fn managed_directive_index(line: &str) -> Option<usize> {
    let (keyword, _) = directive(line)?;
    MANAGED_DIRECTIVES
        .iter()
        .position(|managed| keyword.eq_ignore_ascii_case(managed))
}

fn is_directive(line: &str, expected: &str) -> bool {
    directive(line).is_some_and(|(keyword, _)| keyword.eq_ignore_ascii_case(expected))
}

fn directive(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_start();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let keyword_end = line
        .find(|character: char| character.is_whitespace() || character == '=')
        .unwrap_or(line.len());
    let keyword = &line[..keyword_end];
    let mut value = line[keyword_end..].trim_start();
    if let Some(rest) = value.strip_prefix('=') {
        value = rest.trim_start();
    }
    Some((keyword, strip_inline_comment(value).trim_end()))
}

fn strip_inline_comment(value: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    let mut previous_was_whitespace = true;

    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            previous_was_whitespace = character.is_whitespace();
            continue;
        }
        if character == '\\' {
            escaped = true;
            previous_was_whitespace = false;
            continue;
        }
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        } else if character == '#' && previous_was_whitespace {
            return &value[..index];
        }
        previous_was_whitespace = character.is_whitespace();
    }
    value
}

fn config_lines(contents: &str) -> Vec<ConfigLine<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for full in contents.split_inclusive('\n') {
        let without_newline = full.strip_suffix('\n').unwrap_or(full);
        let text = without_newline
            .strip_suffix('\r')
            .unwrap_or(without_newline);
        lines.push(ConfigLine { full, text, start });
        start += full.len();
    }
    lines
}

fn line_ending(full: &str) -> &str {
    if full.ends_with("\r\n") {
        "\r\n"
    } else if full.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

fn preferred_newline(contents: &str) -> &'static str {
    if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn canonical_group_line(group: &str) -> String {
    canonical_group_line_with_ending(group, "\n", !group.is_empty())
}

fn canonical_group_line_with_ending(group: &str, ending: &str, include_when_empty: bool) -> String {
    if group.is_empty() {
        if include_when_empty {
            format!("# ssh-tray:group{ending}")
        } else {
            String::new()
        }
    } else {
        format!("# ssh-tray:group {group}{ending}")
    }
}

fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

// An empty directive resets grouping for subsequent Host lines.
fn parse_group_directive(line: &str) -> Option<Option<String>> {
    let comment = line.trim_start().strip_prefix('#')?.trim_start();
    let (keyword, value) = comment
        .split_once(char::is_whitespace)
        .unwrap_or((comment, ""));
    if !keyword.eq_ignore_ascii_case("ssh-tray:group") {
        return None;
    }

    let group = value.trim();
    Some((!group.is_empty()).then(|| group.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{Host, HostDraft, delete_host_at, parse_hosts, primary_hosts, save_host_at};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ssh-tray-config-test-{}-{}",
                std::process::id(),
                TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn config_path(&self) -> PathBuf {
            self.0.join(".ssh").join("config")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn draft(aliases: &str) -> HostDraft {
        HostDraft {
            aliases: aliases.to_owned(),
            host_name: "example.com".to_owned(),
            user: "alice".to_owned(),
            port: "22".to_owned(),
            identity_file: "~/.ssh/id_ed25519".to_owned(),
            group: String::new(),
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn aliases(hosts: &[Host]) -> Vec<&str> {
        hosts.iter().map(|host| host.alias.as_str()).collect()
    }

    #[test]
    fn display_keeps_one_primary_alias_per_host_block() {
        let hosts = primary_hosts(parse_hosts(
            "Host studio-main studio-alt studio-backup\n    HostName 192.0.2.10\n\
             Host dgx-main dgx-alt\n    HostName 192.0.2.30\n\
             Host mini-lan\n    HostName 192.0.2.40\n",
        ));

        assert_eq!(aliases(&hosts), ["studio-main", "dgx-main", "mini-lan"]);
        assert_eq!(
            hosts[0].aliases,
            ["studio-main", "studio-alt", "studio-backup"]
        );
        assert_eq!(hosts[1].aliases, ["dgx-main", "dgx-alt"]);
    }

    #[test]
    fn parses_host_lines_case_insensitively_in_file_order() {
        let config = r#"
            # Servers
            Host alpha beta
                HostName alpha.example.com
            hOsT gamma delta
            Hostname ignored.example.com
        "#;

        assert_eq!(
            aliases(&parse_hosts(config)),
            ["alpha", "beta", "gamma", "delta"]
        );
    }

    #[test]
    fn stops_at_inline_comments_and_ignores_comment_lines() {
        let config = r#"
            # Host not-a-host
            Host alpha beta # gamma delta
               # Host also-not-a-host
            Host epsilon
        "#;

        assert_eq!(aliases(&parse_hosts(config)), ["alpha", "beta", "epsilon"]);
    }

    #[test]
    fn excludes_wildcard_and_negated_patterns() {
        let config = r#"
            Host * *.example.com web-? !blocked literal!mark alpha
        "#;

        assert_eq!(aliases(&parse_hosts(config)), ["literal!mark", "alpha"]);
    }

    #[test]
    fn removes_duplicates_without_changing_first_seen_order() {
        let config = r#"
            Host beta alpha beta
            Host gamma alpha beta delta
        "#;

        assert_eq!(
            aliases(&parse_hosts(config)),
            ["beta", "alpha", "gamma", "delta"]
        );
    }

    #[test]
    fn parses_all_aliases_and_managed_values_for_each_host_entry() {
        let hosts = parse_hosts(
            "Host alpha beta !negated *.wildcard\n\
             \tHostName alpha.internal # effective name\n\
             \tUser deploy\n\
             \tPort 2202\n\
             \tIdentityFile \"~/keys/team key\"\n",
        );

        assert_eq!(hosts.len(), 2);
        for host in &hosts {
            assert_eq!(host.aliases, ["alpha", "beta"]);
            assert_eq!(host.host_name, "alpha.internal");
            assert_eq!(host.user, "deploy");
            assert_eq!(host.port, "2202");
            assert_eq!(host.identity_file, "\"~/keys/team key\"");
        }
    }

    #[test]
    fn managed_values_are_scoped_to_their_host_block() {
        let hosts = parse_hosts(
            "HostName global.example\nUser global\nInclude ~/.ssh/conf.d/*\n\
             Host alpha\nHostName alpha.example\nMatch all\nUser matched\n\
             Host beta\nPort 2022\n",
        );

        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].host_name, "alpha.example");
        assert_eq!(hosts[0].user, "");
        assert_eq!(hosts[1].host_name, "");
        assert_eq!(hosts[1].user, "");
        assert_eq!(hosts[1].port, "2022");
    }

    #[test]
    fn assigns_network_groups_until_the_next_directive() {
        let config = r#"
            # ssh-tray:group Office
            Host macstudio studio
            #SSH-TRAY:GROUP Cloudflare
            Host tunnel-mac
            # ssh-tray:group
            Host ungrouped
        "#;

        let hosts = parse_hosts(config);
        let groups: Vec<(&str, Option<&str>)> = hosts
            .iter()
            .map(|host| (host.alias.as_str(), host.group.as_deref()))
            .collect();
        assert_eq!(
            groups,
            [
                ("macstudio", Some("Office")),
                ("studio", Some("Office")),
                ("tunnel-mac", Some("Cloudflare")),
                ("ungrouped", None),
            ]
        );
    }

    #[test]
    fn create_edit_delete_roundtrip_uses_only_the_supplied_temp_path() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        let initial = HostDraft {
            group: "Office".to_owned(),
            ..draft("mac studio")
        };
        save_host_at(&path, None, &initial).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# ssh-tray:group Office\n\
             Host mac studio\n\
             \x20   HostName example.com\n\
             \x20   User alice\n\
             \x20   Port 22\n\
             \x20   IdentityFile ~/.ssh/id_ed25519\n"
        );

        let edited = HostDraft {
            aliases: "studio mac".to_owned(),
            host_name: "studio.internal".to_owned(),
            user: "bob".to_owned(),
            port: "2222".to_owned(),
            identity_file: String::new(),
            group: "Cloud".to_owned(),
        };
        save_host_at(&path, Some("studio"), &edited).unwrap();
        let hosts = parse_hosts(&fs::read_to_string(&path).unwrap());
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].alias, "studio");
        assert_eq!(hosts[0].aliases, ["studio", "mac"]);
        assert_eq!(hosts[0].group.as_deref(), Some("Cloud"));
        assert_eq!(hosts[0].host_name, "studio.internal");
        assert_eq!(hosts[0].identity_file, "");

        delete_host_at(&path, "mac").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        assert!(directory.0.join(".ssh").exists());
    }

    #[test]
    fn create_appends_without_disturbing_an_unterminated_existing_file() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(&path, "Host existing\n    HostName existing.example");
        save_host_at(&path, None, &draft("added")).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Host existing\n    HostName existing.example\n\
             Host added\n\
             \x20   HostName example.com\n\
             \x20   User alice\n\
             \x20   Port 22\n\
             \x20   IdentityFile ~/.ssh/id_ed25519\n"
        );
    }

    #[test]
    fn update_preserves_advanced_options_comments_globals_and_include() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        let original = "Include ~/.ssh/conf.d/*.conf\n\
                        HostName global.invalid\n\
                        # ssh-tray:group Office\n\
                        # host-specific explanation\n\
                        Host jump app old-alias\n\
                        \tHostName old.internal\n\
                        \tProxyJump bastion\n\
                        \tProxyCommand ssh gateway -W %h:%p\n\
                        \tRemoteCommand tmux new -A -s work\n\
                        \tControlMaster auto\n\
                        \tControlPath ~/.ssh/control/%r@%h:%p\n\
                        \tControlPersist 10m\n\
                        \tUser old-user\n\
                        \t# keep this comment\n\
                        \tPort 22\n\
                        \tIdentityFile ~/.ssh/old-key\n\
                        Host untouched\n\
                        \tHostName untouched.example\n\
                        \tProxyJump other-bastion\n";
        write(&path, original);
        let edited = HostDraft {
            aliases: "app new-alias".to_owned(),
            host_name: "new.internal".to_owned(),
            user: "new-user".to_owned(),
            port: "2200".to_owned(),
            identity_file: "~/.ssh/new-key".to_owned(),
            group: "Production".to_owned(),
        };

        save_host_at(&path, Some("old-alias"), &edited).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Include ~/.ssh/conf.d/*.conf\n\
             HostName global.invalid\n\
             # ssh-tray:group Production\n\
             # host-specific explanation\n\
             Host app new-alias\n\
             \x20   HostName new.internal\n\
             \tProxyJump bastion\n\
             \tProxyCommand ssh gateway -W %h:%p\n\
             \tRemoteCommand tmux new -A -s work\n\
             \tControlMaster auto\n\
             \tControlPath ~/.ssh/control/%r@%h:%p\n\
             \tControlPersist 10m\n\
             \x20   User new-user\n\
             \t# keep this comment\n\
             \x20   Port 2200\n\
             \x20   IdentityFile ~/.ssh/new-key\n\
             # ssh-tray:group Office\n\
             Host untouched\n\
             \tHostName untouched.example\n\
             \tProxyJump other-bastion\n"
        );
    }

    #[test]
    fn update_adds_missing_managed_lines_and_removes_cleared_and_duplicate_lines() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(
            &path,
            "Host app\n    ProxyJump bastion\n    User first\n    User second\n    Port 22\n",
        );
        let edited = HostDraft {
            aliases: "app".to_owned(),
            host_name: "app.internal".to_owned(),
            user: String::new(),
            port: String::new(),
            identity_file: "~/.ssh/app".to_owned(),
            group: String::new(),
        };

        save_host_at(&path, Some("app"), &edited).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Host app\n    HostName app.internal\n    IdentityFile ~/.ssh/app\n    ProxyJump bastion\n"
        );
    }

    #[test]
    fn update_can_clear_a_group_without_inheriting_an_earlier_group() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(
            &path,
            "# ssh-tray:group Office\nHost app\n    ProxyJump bastion\n",
        );
        let edited = HostDraft {
            group: String::new(),
            ..draft("app")
        };

        save_host_at(&path, Some("app"), &edited).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("# ssh-tray:group\nHost app\n"));
        assert_eq!(parse_hosts(&contents)[0].group, None);
    }

    #[test]
    fn delete_preserves_comments_and_whitespace_belonging_to_the_next_block() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(
            &path,
            "Host first\n    HostName first.example\n\n\
             # ssh-tray:group Production\n\
             # Jump settings for the production host\n\
             Host second\n\
             \tHostName second.example\n\
             \tProxyJump bastion\n",
        );

        delete_host_at(&path, "first").unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "\n# ssh-tray:group Production\n\
             # Jump settings for the production host\n\
             Host second\n\
             \tHostName second.example\n\
             \tProxyJump bastion\n"
        );
    }

    #[test]
    fn deleting_one_alias_removes_its_entire_multi_alias_block_only() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(
            &path,
            "Host alpha beta\n    HostName alpha.example\n    ProxyJump gateway\nHost gamma\n    HostName gamma.example\n",
        );

        delete_host_at(&path, "beta").unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Host gamma\n    HostName gamma.example\n"
        );
    }

    #[test]
    fn editing_and_deleting_a_host_preserves_following_group_membership() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(
            &path,
            "# ssh-tray:group Office\n\
             Host first\n\
                 HostName first.example\n\
             # Second host documentation\n\
             Host second\n\
                 HostName second.example\n",
        );
        let mut edited = draft("first");
        edited.group = "LAN".to_owned();
        save_host_at(&path, Some("first"), &edited).unwrap();

        let hosts = parse_hosts(&fs::read_to_string(&path).unwrap());
        assert_eq!(hosts[0].group.as_deref(), Some("LAN"));
        assert_eq!(hosts[1].group.as_deref(), Some("Office"));

        delete_host_at(&path, "first").unwrap();
        let hosts = parse_hosts(&fs::read_to_string(&path).unwrap());
        assert_eq!(aliases(&hosts), ["second"]);
        assert_eq!(hosts[0].group.as_deref(), Some("Office"));
    }

    #[test]
    fn update_preserves_crlf_on_unmanaged_lines() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(
            &path,
            "Host app\r\n\tHostName old\r\n\tProxyJump gateway\r\n",
        );
        let edited = HostDraft {
            host_name: "new".to_owned(),
            ..draft("app")
        };

        save_host_at(&path, Some("app"), &edited).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Host app\r\n    User alice\r\n    Port 22\r\n    IdentityFile ~/.ssh/id_ed25519\r\n    HostName new\r\n\tProxyJump gateway\r\n"
        );
    }

    #[test]
    fn rejects_unsafe_aliases_invalid_ports_and_newlines_without_writing() {
        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(&path, "Host safe\n    ProxyJump gateway\n");
        let original = fs::read_to_string(&path).unwrap();

        for aliases in [
            "",
            "*",
            "bad?",
            "!negated",
            "#comment",
            "-option",
            "bad!alias",
            "semi;colon",
        ] {
            let mut invalid = draft("safe");
            invalid.aliases = aliases.to_owned();
            assert!(save_host_at(&path, Some("safe"), &invalid).is_err());
        }
        for port in ["0", "65536", "-1", "ssh"] {
            let mut invalid = draft("safe");
            invalid.port = port.to_owned();
            assert!(save_host_at(&path, Some("safe"), &invalid).is_err());
        }
        for field in 0..5 {
            let mut invalid = draft("safe");
            match field {
                0 => invalid.host_name = "good\nHost injected".to_owned(),
                1 => invalid.user = "good\rHost injected".to_owned(),
                2 => invalid.identity_file = "good\nHost injected".to_owned(),
                3 => invalid.group = "good\nHost injected".to_owned(),
                _ => invalid.port = "22\nHost injected".to_owned(),
            }
            assert!(save_host_at(&path, Some("safe"), &invalid).is_err());
        }

        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert!(delete_host_at(&path, "missing").is_err());
        assert!(save_host_at(&path, Some("missing"), &draft("new")).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_replacement_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDirectory::new();
        let path = directory.config_path();
        write(&path, "Host app\n    HostName old\n");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        save_host_at(&path, Some("app"), &draft("app")).unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>(),
            ["config"],
        );
    }
}
