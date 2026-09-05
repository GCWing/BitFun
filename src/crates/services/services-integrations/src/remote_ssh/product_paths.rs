//! Product-owned POSIX paths used on remote SSH hosts.
//!
//! Remote paths must use POSIX separators even when the controller runs on
//! Windows, so they cannot be assembled with `std::path::Path`.

use openbitfun_services_core::product_identity::hidden_data_directory;

fn data_relative_path(hidden_directory: &str, segments: &[&str]) -> String {
    let mut path = hidden_directory.trim_matches('/').to_string();
    for segment in segments {
        let segment = segment.trim_matches('/');
        if segment.is_empty() {
            continue;
        }
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(segment);
    }
    path
}

pub(crate) fn product_data_relative_path(segments: &[&str]) -> String {
    data_relative_path(hidden_data_directory(), segments)
}

pub(crate) fn product_data_path(base: &str, segments: &[&str]) -> String {
    let relative = product_data_relative_path(segments);
    let trimmed_base = base.trim_end_matches('/');
    if trimmed_base.is_empty() {
        if base.starts_with('/') {
            format!("/{relative}")
        } else {
            relative
        }
    } else {
        format!("{trimmed_base}/{relative}")
    }
}

pub(crate) fn product_home_shell_path(segments: &[&str]) -> String {
    format!("$HOME/{}", product_data_relative_path(segments))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_remote_paths_use_the_compiled_product_identity() {
        assert_eq!(
            product_data_path("/home/user", &["dispatch", "install"]),
            "/home/user/.openbitfun/dispatch/install"
        );
        assert_eq!(
            product_home_shell_path(&["relay-deploy", "relay.port"]),
            "$HOME/.openbitfun/relay-deploy/relay.port"
        );
    }

    #[test]
    fn path_assembly_does_not_embed_the_default_namespace() {
        assert_eq!(
            data_relative_path(".acme", &["dispatch", "requests"]),
            ".acme/dispatch/requests"
        );
    }

    #[test]
    fn root_base_keeps_its_leading_separator() {
        assert_eq!(product_data_path("/", &["bin"]), "/.openbitfun/bin");
    }
}
