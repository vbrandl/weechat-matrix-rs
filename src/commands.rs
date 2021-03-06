use clap::App as Argparse;
use clap::AppSettings as ArgParseSettings;
use clap::{Arg, ArgMatches, SubCommand};
use url::Url;

use crate::config::Config;
use crate::PLUGIN_NAME;
use crate::{MatrixServer, Servers};
use weechat::buffer::Buffer;
use weechat::hooks::{Command, CommandCallback, CommandSettings};
use weechat::{ArgsWeechat, Weechat};

pub struct Commands {
    _matrix: Command,
}

struct MatrixCommand {
    servers: Servers,
    config: Config,
}

impl MatrixCommand {
    fn add_server(&self, args: &ArgMatches) {
        let server_name = args
            .value_of("name")
            .expect("Server name not set but was required");
        let homeserver = args
            .value_of("homeserver")
            .expect("Homeserver not set but was required");
        let homeserver = Url::parse(homeserver)
            .expect("Can't parse Homeserver even if validation passed");

        let mut config_borrow = self.config.borrow_mut();
        let mut section = config_borrow
            .search_section_mut("server")
            .expect("Can't get server section");

        let server = MatrixServer::new(server_name, &self.config, &mut section);

        let mut servers = self.servers.borrow_mut();
        servers.insert(server_name.to_owned(), server);

        let homeserver_option = section
            .search_option(&format!("{}.homeserver", server_name))
            .expect("Homeserver option wasn't created");
        homeserver_option.set(homeserver.as_str(), true);

        Weechat::print(&format!(
            "{}: Server {}{}{} has been added.",
            PLUGIN_NAME,
            Weechat::color("chat_server"),
            server_name,
            Weechat::color("reset")
        ));
    }

    fn delete_server(&self, args: &ArgMatches) {
        let server_name = args
            .value_of("name")
            .expect("Server name not set but was required");

        let mut servers = self.servers.borrow_mut();

        let connected = {
            let server = servers.get(server_name);

            if let Some(s) = server {
                s.connected()
            } else {
                Weechat::print(&format!(
                    "{}: No such server {}{}{} found.",
                    PLUGIN_NAME,
                    Weechat::color("chat_server"),
                    server_name,
                    Weechat::color("reset")
                ));
                return;
            }
        };

        if connected {
            Weechat::print(&format!(
                "{}: Server {}{}{} is still connected.",
                PLUGIN_NAME,
                Weechat::color("chat_server"),
                server_name,
                Weechat::color("reset")
            ));
            return;
        }

        let server = servers.remove(server_name).unwrap();

        drop(server);

        Weechat::print(&format!(
            "{}: Server {}{}{} has been deleted.",
            PLUGIN_NAME,
            Weechat::color("chat_server"),
            server_name,
            Weechat::color("reset")
        ));
    }

    fn list_servers(&self) {
        if self.servers.borrow().is_empty() {
            return;
        }

        Weechat::print("\nAll Matrix servers:");

        // TODO print out some stats if the server is connected.
        for server in self.servers.borrow().keys() {
            Weechat::print(&format!(
                "    {}{}",
                Weechat::color("chat_server"),
                server
            ));
        }
    }

    fn server_command(&self, args: &ArgMatches) {
        match args.subcommand() {
            ("add", Some(subargs)) => self.add_server(subargs),
            ("delete", Some(subargs)) => self.delete_server(subargs),
            ("list", _) => self.list_servers(),
            _ => self.list_servers(),
        }
    }

    fn server_not_found(&self, server_name: &str) {
        Weechat::print(&format!(
            "{}{}: Server \"{}{}{}\" not found.",
            Weechat::prefix("error"),
            PLUGIN_NAME,
            Weechat::color("chat_server"),
            server_name,
            Weechat::color("reset")
        ));
    }

    fn connect_command(&self, args: &ArgMatches) {
        let server_names = args
            .values_of("name")
            .expect("Server names not set but were required");

        let mut servers = self.servers.borrow_mut();

        for server_name in server_names {
            let server = servers.get_mut(server_name);
            if let Some(s) = server {
                match s.connect() {
                    Ok(_) => (),
                    Err(e) => Weechat::print(&format!("{:?}", e)),
                }
            } else {
                self.server_not_found(server_name)
            }
        }
    }

    fn disconnect_command(&self, args: &ArgMatches) {
        let mut servers = self.servers.borrow_mut();

        let server_name = args
            .value_of("name")
            .expect("Server name not set but was required");

        let server = servers.get_mut(server_name);

        if let Some(s) = server {
            s.disconnect();
        } else {
            self.server_not_found(server_name)
        }
    }
}

impl CommandCallback for MatrixCommand {
    fn callback(
        &mut self,
        _weechat: &Weechat,
        _buffer: &Buffer,
        arguments: ArgsWeechat,
    ) {
        let server_command = SubCommand::with_name("server")
            .about("List, add or delete Matrix servers.")
            .subcommand(
                SubCommand::with_name("add")
                    .about("Add a new Matrix server.")
                    .arg(
                        Arg::with_name("name")
                            .value_name("server-name")
                            .required(true),
                    )
                    .arg(
                        Arg::with_name("homeserver")
                            .required(true)
                            .validator(MatrixServer::parse_homeserver_url),
                    ),
            )
            .subcommand(
                SubCommand::with_name("delete")
                    .about("Delete an existing Matrix server.")
                    .arg(
                        Arg::with_name("name")
                            .value_name("server-name")
                            .required(true),
                    ),
            )
            .subcommand(
                SubCommand::with_name("list")
                    .about("List the configured Matrix servers."),
            );

        let argparse = Argparse::new("matrix")
            .about("Matrix chat protocol command.")
            // .global_setting(ArgParseSettings::ColorNever)
            .global_setting(ArgParseSettings::DisableHelpFlags)
            .global_setting(ArgParseSettings::DisableVersion)
            .global_setting(ArgParseSettings::VersionlessSubcommands)
            .setting(ArgParseSettings::SubcommandRequiredElseHelp)
            .subcommand(server_command)
            .subcommand(
                SubCommand::with_name("connect")
                    .about("Connect to Matrix servers.")
                    .arg(
                        Arg::with_name("name")
                            .value_name("server-name")
                            .required(true)
                            .multiple(true),
                    ),
            )
            .subcommand(
                SubCommand::with_name("disconnect")
                    .about("Disconnect from one or all Matrix servers")
                    .arg(
                        Arg::with_name("name")
                            .value_name("server-name")
                            .required(true),
                    ),
            );

        let matches = match argparse.get_matches_from_safe(arguments) {
            Ok(m) => m,
            Err(e) => {
                Weechat::print(
                    &Weechat::execute_modifier(
                        "color_decode_ansi",
                        "1",
                        &e.to_string(),
                    )
                    .unwrap(),
                );
                return;
            }
        };

        match matches.subcommand() {
            ("connect", Some(subargs)) => self.connect_command(subargs),
            ("disconnect", Some(subargs)) => self.disconnect_command(subargs),
            ("server", Some(subargs)) => self.server_command(subargs),
            _ => unreachable!(),
        }
    }
}

impl Commands {
    pub fn hook_all(
        weechat: &Weechat,
        servers: &Servers,
        config: &Config,
    ) -> Commands {
        let matrix_settings = CommandSettings::new("matrix")
            .description("Matrix chat protocol command.")
            .add_argument("server add <server-name> <hostname>[:<port>]")
            .add_argument("server delete|list|listfull <server-name>")
            .add_argument("connect <server-name>")
            .add_argument("disconnect <server-name>")
            .add_argument("reconnect <server-name>")
            .add_argument("help <matrix-command> [<matrix-subcommand>]")
            .arguments_description(
                "     server: List, add, or remove Matrix servers.
    connect: Connect to Matrix servers.
 disconnect: Disconnect from one or all Matrix servers.
  reconnect: Reconnect to server(s).
       help: Show detailed command help.\n
Use /matrix [command] help to find out more.\n",
            )
            .add_completion("server |add|delete|list|listfull")
            .add_completion("connect")
            .add_completion("disconnect")
            .add_completion("reconnect")
            .add_completion("help server|connect|disconnect|reconnect");

        let matrix = weechat.hook_command(
            matrix_settings,
            MatrixCommand {
                servers: servers.clone(),
                config: config.clone(),
            },
        );

        Commands { _matrix: matrix }
    }
}
