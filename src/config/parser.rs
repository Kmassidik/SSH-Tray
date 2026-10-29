use super::{Host, config_path};
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;

pub(super) const MANAGED_DIRECTIVES: [&str; 4] = ["HostName", "User", "Port", "IdentityFile"];

#[derive(Debug, Clone, Copy)]
pub(super) struct ConfigLine<'a> {
    pub(super) full: &'a str,
    pub(super) text: &'a str,
    pub(super) start: usize,
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

pub(super) fn parse_hosts(contents: &str) -> Vec<Host> {
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

pub(super) fn find_host_block(lines: &[ConfigLine<'_>], alias: &str) -> Option<(usize, usize)> {
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

pub(super) fn group_at_host(lines: &[ConfigLine<'_>], host_index: usize) -> Option<String> {
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

pub(super) fn managed_directive_index(line: &str) -> Option<usize> {
    let (keyword, _) = directive(line)?;
    MANAGED_DIRECTIVES
        .iter()
        .position(|managed| keyword.eq_ignore_ascii_case(managed))
}

pub(super) fn is_directive(line: &str, expected: &str) -> bool {
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

pub(super) fn config_lines(contents: &str) -> Vec<ConfigLine<'_>> {
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

pub(super) fn line_ending(full: &str) -> &str {
    if full.ends_with("\r\n") {
        "\r\n"
    } else if full.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

pub(super) fn preferred_newline(contents: &str) -> &'static str {
    if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

pub(super) fn canonical_group_line(group: &str) -> String {
    canonical_group_line_with_ending(group, "\n", !group.is_empty())
}

pub(super) fn canonical_group_line_with_ending(
    group: &str,
    ending: &str,
    include_when_empty: bool,
) -> String {
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

pub(super) fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

// An empty directive resets grouping for subsequent Host lines.
pub(super) fn parse_group_directive(line: &str) -> Option<Option<String>> {
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
    use super::{parse_hosts, primary_hosts};
    use crate::config::Host;

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
}
