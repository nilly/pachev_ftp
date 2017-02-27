//! FTP Server implemented in rust for CNT4713 Net Centric a
//! Florida International University

extern crate argparse; //argument parsing such as -h -d etc..
extern crate rand;
#[macro_use]
extern crate log;

//For ignore case
use std::ascii::AsciiExt;

use std::io::prelude::*; //the standard io functions that come with rust
use std::io::BufReader;
use std::thread::spawn; //For threads

use std::string::String;
use std::net::{Ipv4Addr, TcpStream, TcpListener, Shutdown, SocketAddrV4};

use std::path::Path;
use std::fs;
use std::fs::File;

use std::env; //To collect arguements and variables
// use std::process::exit; //Gracefully exiting
use std::iter::Iterator;
use std::collections::HashMap;

use argparse::{ArgumentParser, Print, Store, StoreOption, StoreTrue};

//TODO implement this: https://github.com/Evrey/passwors#passwors-usage
//TODO: Logger for rust is iumplemented using the log crate https://doc.rust-lang.org/log/log/index.html


// Local Modules
mod server;
mod user;
mod main_commands;

use user::User;
use server::FtpMode;
use main_commands as mc;

#[derive(Debug, Clone)]
struct Arguements {
    ftp_port: String,
    service_port: String,
    passive: bool,
    active: bool,
    debug: bool,
    verbose: bool,
    data_port_range: String,
    run_test_file: Option<String>,
    config_file: Option<String>,
    log_file: Option<String>,
}

//These are the defaults incase no arguements are provided
impl Arguements {
    /// A struct that handles all the command line arguements
    ///
    /// # Otions supported supported
    /// - `-h` for hostname
    /// - `-p` for port
    /// - `--pasive` to enable passive mode
    ///
    /// # Remarks
    ///
    /// It was easier to use a tested crate rather than parsing argument myself
    fn new() -> Arguements {
        Arguements {
            ftp_port: "2115".to_string(),
            service_port: "2185".to_string(),
            passive: false,
            active: false,
            debug: false,
            verbose: false,
            data_port_range: "27500-27999".to_string(),
            run_test_file: None,
            config_file: None,
            log_file: None,
        }
    }
}

fn main() {

    let mut arguements = Arguements::new();

    {

        let mut ap = ArgumentParser::new();
        ap.set_description("Pachev's FTP client");

        ap.add_option(&["--info", "-i", "--list-commands"],
                      Print(COMMANDS_HELP.to_string()),
                      "List supported commands");
        ap.add_option(&["--version", "-v"],
                      Print("v0.1.0".to_string()),
                      "Prints version");

        ap.refer(&mut arguements.ftp_port)
            .add_option(&["--port", "-p"], Store, "Server Port");

        ap.refer(&mut arguements.passive)
            .add_option(&["--pasive"],
                        StoreTrue,
                        "Use passive mode and 
                                listen on \
                         provided address for data transfers");
        ap.refer(&mut arguements.active)
            .add_option(&["--active"],
                        StoreTrue,
                        "Use active mode and 
                                listen on provided \
                         address for data transfers");
        ap.refer(&mut arguements.debug)
            .add_option(&["-D", "--debug"], StoreTrue, "Sets debug mode on");

        ap.refer(&mut arguements.verbose)
            .add_option(&["-V", "--verbose"], StoreTrue, "Sets verbose  mode on");

        ap.refer(&mut arguements.data_port_range)
            .add_option(&["--dpr"], Store, "Sets a range of ports for data");

        ap.refer(&mut arguements.config_file)
            .add_option(&["-c", "--config"],
                        StoreOption,
                        "location of configuration file");

        ap.refer(&mut arguements.run_test_file)
            .add_option(&["-t", "--test-file"], StoreOption, "location of test file");

        ap.parse_args_or_exit();
    }

    // create_root();

    let server = format!("127.0.0.1:{}", arguements.ftp_port);
    let listener = TcpListener::bind(server.as_str()).expect("Could not bind to socket");
    let data_port_range = get_data_ports(format!("{}", arguements.data_port_range));

    let mut users: HashMap<String, user::User> = HashMap::new();
    users = get_user_list();


    println!("Welcome to Pachev's Famous Rusty FTP Server");
    let mut port_count = 0;

    for stream in listener.incoming() {
        let data_port = data_port_range[port_count];
        let mut stream = stream.expect("Could not create TCP Stream");


        //Eventually this is schanged to logger and then printed based on preferences
        info!("DEBUG: client {} has started and given data port {}",
              stream.peer_addr().unwrap().ip(),
              data_port);

        let mut map = users.clone();

        spawn(move || {
            let mut b_stream = BufReader::new(stream);
            handle_client(&mut b_stream, &data_port, &mut map);
        });
        port_count += 1;
    }
}

/// # handle_client
///
/// This is the main function that handles the client thread
///
/// # Arguements
///
/// - client
/// - data_port
/// - map
fn handle_client(mut client: &mut BufReader<TcpStream>,
                 data_port: &i32,
                 mut map: &HashMap<String, user::User>) {

    let data_server = format!("{}:{}",
                              client.get_mut().local_addr().unwrap().ip(),
                              data_port);

    let mut actv_socket_addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 27598);

    let mut data_type = String::new();
    let mut data_listener = TcpListener::bind(data_server.as_str())
        .expect("Could not open data serve");
    let mut ftp_mode = FtpMode::Passive;
    let mut logged_in = false;
    let mut limit = 3; //TODO: add this in the configuration file
    let mut msg = String::new();
    let mut user = User::new();

    msg = format!("{} Pachev's FTP Server {}\r\n",
                  server::LOGGED_EXPECTED,
                  client.get_mut().local_addr().unwrap().ip());


    server::write_response(&mut client, &msg);

    loop {

        let mut response = String::new();
        client.read_line(&mut response).expect("Could not read stream message");

        let line = response.trim();

        let (cmd, args) = match line.find(' ') {
            Some(pos) => (&line[0..pos], &line[pos + 1..]),
            None => (line, "".as_ref()),
        };
        println!("CLIENT: {} {}", cmd, args);

        //TODO: Fix logic with logged_in so I'm not repatedly checking for each pattern

        match cmd.to_lowercase().as_ref() {
            "user" => {
                if !logged_in {
                    match server::handle_user(&mut client, &args, &map) {
                        true => {
                            logged_in = true;
                            user = map.get(args).unwrap().clone();
                        }
                        false => {
                            logged_in = false;
                            limit -= 1;
                            if limit <= 0 {
                                break;
                            }
                        }
                    }
                } else {
                    server::write_response(client,
                                           &format!("{} Badd sequence of commands\r\n",
                                                    server::NOT_UNDERSTOOD));
                }
            }
            "appe" => {
                if logged_in {
                    mc::stor(&mut client, &user, ftp_mode, &args, &data_listener);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "cdup" => {
                if logged_in {
                    server::cdup(&mut client, &mut user);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "cwd" | "cd" => {
                if logged_in {
                    server::cwd(&mut client, &args, &mut user);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "dele" => {
                if logged_in {
                    mc::dele(&mut client, &user, &args);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "list" => {
                if logged_in {
                    mc::list(&mut client,
                             &user,
                             ftp_mode,
                             &args,
                             &data_port,
                             &data_listener);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "mkd" | "mkdir" => {
                if logged_in {
                    server::mkd(&mut client, &args, &mut user);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "pasv" => {
                if logged_in {
                    server::handle_mode(&mut client, ftp_mode, &data_port, &args);

                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "port" => {
                if logged_in {
                    actv_socket_addr = port_addr(args);
                    ftp_mode = FtpMode::Active(actv_socket_addr);

                    server::handle_mode(&mut client, ftp_mode, &data_port, &args);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "retr" => {
                if logged_in {
                    mc::retr(&mut client, &user, ftp_mode, &args, &data_listener);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "rnfr" => {
                if logged_in {
                    mc::rnfr(&mut client, &user, &args);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "stor" => {
                if logged_in {
                    mc::stor(&mut client, &user, ftp_mode, &args, &data_listener);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "stou" => {
                if logged_in {
                    mc::stou(&mut client, &user, ftp_mode, &args, &data_listener);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "type" => {
                if logged_in {
                    server::handle_type(&mut client, &args);
                } else {
                    server::write_response(&mut client,
                                           &format!("{} Not Logged In\r\n",
                                                    server::AUTHENTICATION_FAILED));
                }
            }
            "quit" | "exit" | "logout" => {
                server::write_response(&mut client, &format!("{} GOODBYE\r\n", server::GOODBYE));
                break;
            }
            "syst" => {
                //TODO: Make system  ox agnostic
                server::write_response(&mut client,
                                       &format!("{} UNIX Type: L8r\n", server::SYSTEM_RECEIVED));
            }
            "help" | "?" => server::write_response(&mut client, &COMMANDS_HELP),
            _ => server::write_response(&mut client, &format!("Invalid Command\r\n")),
        }

    }
    //TODO: A mutex should go here but rust handles variables iwth 'Move"
    //The mutex will decrement the total client count

    client.get_mut().shutdown(Shutdown::Both).expect("couldn't close server");
    println!("Client {} has closed connection", data_port - 27500);
}


// Initializes a users here
fn initialize_user(name: &str, pass: &str, role: &str) -> User {
    //Figuring out the current dirrectory
    let cur_directory = match env::current_dir() {
        Ok(pwd) => format!("{}", pwd.display()).to_string(),
        //Assigns to tmp if it doesn't exist
        Err(err) => format!("/tmp/").to_string(),

    };

    let temp = format!("{}/ftproot/{}", cur_directory, name);
    let user_path = Path::new(&temp);

    if !user_path.exists() {
        fs::create_dir_all(&temp).expect("Could not create user director");
    }
    let mut user = User::new();
    user.name = format!("{}", name).to_string();
    user.role = format!("{}", role).to_string();
    user.pass = format!("{}", pass).to_string();
    user.path = format!("{}", temp).to_string();
    user.cur_dir = format!("{}", temp).to_string();

    return user;
}


//takes the command line argument in the form of 1-5 and returns array of ports
fn get_data_ports(ports: String) -> Vec<i32> {
    //Split the range in order to have an array of ports to issue
    let port_str_range: Vec<&str> = ports.trim().split('-').collect();
    let init_port: i32 = port_str_range[0].parse::<i32>().expect("could not parse ports");
    let last_port: i32 = port_str_range[1].parse::<i32>().expect("could not parse ports");

    let mut port_int_range: Vec<i32> = Vec::new();

    for i in init_port..last_port + 1 {
        port_int_range.push(i);
    }

    return port_int_range;

}

//OPTIMIZE<Possible>
fn get_user_list() -> HashMap<String, user::User> {
    let cur_directory = match env::current_dir() {
        Ok(pwd) => format!("{}", pwd.display()).to_string(),
        //Assigns to tmp if it doesn't exist
        Err(err) => format!("/tmp/").to_string(),

    };

    let mut map: HashMap<String, user::User> = HashMap::new();

    let user_list = format!("{}/users.cfg", cur_directory);
    let f = File::open(user_list).expect("Could not open user files");
    let mut file = BufReader::new(f);
    // let mut users: Vec<&str> = Vec::new(); //May still user as alternative
    let mut user = user::User::new();

    for line in file.lines() {
        let line = line.expect("Could not read line");
        let (things, thrash) = match line.find('#') {
            Some(pos) => (line[0..pos].to_string(), line[pos + 1..].to_string()),
            None => (line, "".to_string()),
        };

        if things.is_empty() {
            continue;
        }

        let split = things.split(' ');
        let tokens: Vec<&str> = split.collect();
        user = initialize_user(&tokens[0].to_string(),
                               &tokens[1].to_string(),
                               &tokens[2].to_string());
        let name = tokens[0].to_string();
        println!("name: {}, role {}", name, tokens[2]);
        map.insert(name, user);
    }

    map

}

fn port_addr(args: &str) -> SocketAddrV4 {
    let nums: Vec<u8> = args.split(',').map(|x| x.parse::<u8>().unwrap()).collect();
    let ip = Ipv4Addr::new(nums[0], nums[1], nums[2], nums[3]);
    let port = server::to_ftp_port(nums[4] as u16, nums[5] as u16);
    let addr = SocketAddrV4::new(ip, port);
    addr

}


const COMMANDS_HELP: &'static str =
    "
Pachev Joseph - 5699044
FTP Server- V0.1.0
use --help for help on starting client
Commands: 
        \
     user - Sends the username
        pass - Send the password
        cwd - Changes working \
     directory
        cdup - Changes to parent directory
        logout - Terminates session
        \
     retr - Retrieves a file
        stor - Stores a file
        stou - Stores a file uniquely
        \
     appe - Appends to a file
        type - Stes tranfer type to Active or Passive
        rnrf \
     - Rename From
        rnto - Rename To
        abor - Aborts a transfer
        dele - \
     Deletes a file
        rmd - Removes a directory
        mkd - Makes a directory
        pwd \
     - Prints working directory
        list - Lists files
        noop - Does nothing
        \
     help - Prints Help Menu
     size - Prints size of file
     nlist - Name list of \
     diretory\r\n
        ";
