use anyhow::{anyhow, Result};
use camloc_common::{
    choice,
    cv::{self, display_image},
    get_from_stdin,
    hosts::{HostState, HostType},
    position::{calc_position_in_square_distance, get_camera_distance_in_square},
    yes_no_choice, Position,
};
use camloc_organizer::{
    CalibrationInterface, Host, ImageStreamInterface, Organizer, OrganizerInterface,
};

#[derive(Debug, Clone, Copy)]
enum SetupType {
    Square { side_length: f64 },
    Free,
}

impl SetupType {
    fn select_camera_position(&self, fov: f64) -> Result<Position> {
        println!("Enter camera position");
        Ok(match self {
            SetupType::Square { side_length } => calc_position_in_square_distance(
                get_from_stdin("  Camera index: ")?,
                get_camera_distance_in_square(*side_length, fov),
            ),
            SetupType::Free => Position::new(
                get_from_stdin("  x: ")?,
                get_from_stdin("  y: ")?,
                get_from_stdin::<f64>("  rotation (degrees): ")?.to_radians(),
            ),
        })
    }
    fn get() -> Result<Self> {
        match choice(
            [("Square", true), ("Free", true)].into_iter(),
            Some("Select setup type: "),
            Some(1),
        )? {
            0 => Ok(SetupType::Square {
                side_length: get_from_stdin("Enter side length: ")?,
            }),

            1 => Ok(SetupType::Free),
            _ => Err(anyhow!("Invalid index")),
        }
    }
}

macro_rules! get_hosts {
    ($organizer:ident, $pat:pat) => {{
        let options: Vec<(&Host, bool)> = $organizer
            .hosts()
            .iter()
            .filter_map(|h| {
                let i = h.info();

                if matches!((i.host_type, i.host_state), $pat) {
                    Some((h, true))
                } else {
                    None
                }
            })
            .collect();
        options
    }};
}
macro_rules! choose_host {
    ($organizer:ident, $pat:pat) => {{
        let options = get_hosts!($organizer, $pat);
        if options.is_empty() {
            println!("No clients found");
            return Ok(());
        }

        let host_index = choice(
            options.iter().cloned(),
            Some("\nSelect client to update: "),
            None,
        )?;

        *options[host_index].0
    }};
}

fn main() -> Result<()> {
    let args = {
        use clap::Parser;

        /// The camloc organizer
        #[derive(Parser)]
        struct Args {
            /// The arcuco ids on the cube (counterclockwise)
            #[arg(short, long, required = true, num_args = 4)]
            cube: Vec<u8>,
        }

        Args::parse()
    };
    let setup = SetupType::get()?;

    let mut buff = [0; 4096];
    let mut organizer = Organizer::start(&mut buff, args.cube.try_into().unwrap())?;

    loop {
        organizer.scan()?;
        handle_commands(&mut organizer, CliInterface { setup })?;
    }
}

struct CliInterface {
    setup: SetupType,
}
impl CliInterface {
    fn more_inner(&self) -> Result<bool> {
        let more = yes_no_choice("  Continue?", false);
        if !more {
            let _ = opencv::highgui::destroy_window("recieved");
        }

        Ok(more)
    }
}

impl OrganizerInterface for CliInterface {
    type CalibrationInterface = Self;
    type ImageStreamInterface = Self;

    type Error = anyhow::Error;

    fn start_image_stream(self) -> Result<Self::ImageStreamInterface, Self::Error> {
        println!("Starting image stream");
        Ok(self)
    }

    fn start_calibration(self) -> Result<Self::CalibrationInterface, Self::Error> {
        println!("Starting calibration");
        Ok(self)
    }
}
impl CalibrationInterface for CliInterface {
    type Parent = Self;

    fn get_board_size(&self) -> Result<(u8, u8), <Self::Parent as OrganizerInterface>::Error> {
        Ok((
            get_from_stdin("  Charuco board width: ")?,
            get_from_stdin("  Charuco board height: ")?,
        ))
    }

    fn keep_image(
        &self,
        img: &opencv::prelude::Mat,
        board: &camloc_common::cv::FoundBoard,
    ) -> Result<bool, <Self::Parent as OrganizerInterface>::Error> {
        let mut img = img.clone();
        cv::draw_board(&mut img, board)?;
        display_image(&img, "recieved", true)?;

        Ok(yes_no_choice("  Keep image?", true))
    }

    fn board_not_found(
        &self,
        img: &opencv::prelude::Mat,
    ) -> Result<(), <Self::Parent as OrganizerInterface>::Error> {
        display_image(img, "recieved", true)?;
        print!("  Board not found\n  ");
        Ok(())
    }

    fn more(&self) -> Result<bool, <Self::Parent as OrganizerInterface>::Error> {
        self.more_inner()
    }

    fn select_camera_position(
        self,
        fov: f64,
    ) -> Result<camloc_common::Position, <Self::Parent as OrganizerInterface>::Error> {
        self.setup.select_camera_position(fov)
    }
}

impl ImageStreamInterface for CliInterface {
    type Parent = Self;

    fn show(
        &self,
        img: &opencv::prelude::Mat,
    ) -> Result<(), <Self::Parent as OrganizerInterface>::Error> {
        display_image(img, "recieved", true)?;
        Ok(())
    }

    fn more(&self) -> Result<bool, <Self::Parent as OrganizerInterface>::Error> {
        self.more_inner()
    }

    fn select_camera_position(
        self,
        fov: f64,
    ) -> Result<camloc_common::Position, <Self::Parent as OrganizerInterface>::Error> {
        self.setup.select_camera_position(fov)
    }
}

fn handle_commands<const BUFFER_SIZE: usize>(
    organizer: &mut Organizer<'_, BUFFER_SIZE>,
    interface: CliInterface,
) -> Result<()> {
    let server = match organizer.get_server() {
        Ok(s) => s,
        Err(_) => {
            println!("No server running");
            return Ok(());
        }
    };

    if let HostState::Idle = server.info().host_state {
        if !yes_no_choice("Server isn't running, do you want to start it?", true) {
            return Ok(());
        }

        organizer.start_server()?;

        return Ok(());
    }

    #[derive(Debug, Clone, Copy)]
    enum OrganizerCommand {
        Start,
        Stop,
        List,
        Scan,
        Update,
        Quit,
    }
    use OrganizerCommand::*;
    const COMMANDS: [OrganizerCommand; 6] = [Start, Stop, List, Scan, Update, Quit];
    impl std::fmt::Display for OrganizerCommand {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }

    let cmd = choice(
        COMMANDS.map(|c| (c, true)).into_iter(),
        Some("Choose action: "),
        None,
    );
    let Ok(cmd) = cmd.map(|i| COMMANDS[i]) else {
        return Ok(());
    };

    println!();

    match cmd {
        Start => {
            let h = choose_host!(organizer, (_, HostState::Idle));
            if let Err(e) = organizer.start_host(h, interface) {
                println!("Couldn't start client because: {e}");
            }
        }
        Stop => {
            let h = choose_host!(organizer, (_, HostState::Running));
            if let Err(e) = organizer.stop_host(h) {
                println!("Couldn't stop client because: {e}");
            }
        }
        List => {
            for h in organizer.hosts() {
                println!("{h}");
            }
        }

        Scan => (),

        Update => {
            let h = choose_host!(organizer, (HostType::Client { .. }, HostState::Running));

            let position = Position::new(
                get_from_stdin("  x: ")?,
                get_from_stdin("  y: ")?,
                get_from_stdin::<f64>("  rotation (degrees): ")?.to_radians(),
            );
            let fov = if yes_no_choice("  Do you also want to change the fov?", false) {
                Some(get_from_stdin::<f64>("  fov (degrees): ")?.to_radians())
            } else {
                None
            };

            organizer.update_info(h, position, fov)?;
        }
        Quit => {
            println!("Quitting...");
            std::process::exit(0)
        }
    }
    println!();

    Ok(())
}
