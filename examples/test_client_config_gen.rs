use camloc_server::calc::{Setup, CameraInfo};

fn main() {
	let args: Vec<String> = std::env::args()
		.skip(1)
		.collect();

	let id = args[1].parse().unwrap();
	let server_addr = "127.0.0.1";
	let port = 1234;

	let setup = Setup::new_square(3., vec![
		CameraInfo::new(62.2f64.to_radians()); 2
	]);

	let cam = setup.cameras[id as usize];

	let ms = args[2].parse().unwrap();

	let buf: Vec<u8> = [
		i32::to_be_bytes(id).to_vec(),

		u16::to_be_bytes(server_addr.len() as u16).to_vec(),
		server_addr.as_bytes().to_vec(),
		i32::to_be_bytes(port).to_vec(),

		f64::to_be_bytes(cam.position.x).to_vec(),
		f64::to_be_bytes(cam.position.y).to_vec(),
		f64::to_be_bytes(cam.position.rotation).to_vec(),
		f64::to_be_bytes(cam.info.fov).to_vec(),

		i64::to_be_bytes(ms).to_vec(),
	].into_iter().flatten().collect();

	std::fs::write(&args[0], buf).unwrap();
}
