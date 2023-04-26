import java.io.FileNotFoundException;
import java.net.InetSocketAddress;
import java.io.DataInputStream;
import java.io.FileInputStream;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.SocketAddress;
import java.net.SocketTimeoutException;
import java.util.ArrayList;
import java.nio.ByteBuffer;
import java.util.Iterator;
import java.util.Arrays;
import java.util.Random;

class Main {

	public record Config(
		int id, SocketAddress serverAddress,
		Vector2 pos, double rot, double fov,
		long ms
	) {}

	record Vector2(double x, double y) {}

	static double clamp(double value, double min, double max) {
		if (value >= max)
			return max;
		if (value <= min)
			return min;
		return value;
	}

	static Vector2 getPoss(Vector2 p, double square_size, double fov) {
		double cd = getCamDistance(square_size, fov);

		double m1 = Math.atan2(p.y, p.x + cd);
		double x1 = 1 - (m1 + fov / 2) / fov;
	
		double m2 = Math.atan(p.x / (p.y - cd));
		double x2 = 1 - (m2 + fov / 2) / fov;

		return new Vector2(x1, x2);
	}

	static enum PositionGenerator {
		SQRT {
			Iterator<Vector2> genPoints() {
				Vector2[] ps = new Vector2[7];
				int a = 1;
				for (int i = 0; i < 7 * a; i++) {
					double x = 0.2 / a * i;
					double y = Math.sqrt(x) / 3;
					ps[i] = getPoss(new Vector2(x, y), 3, Math.toRadians(62.2));
				}

				return Arrays.stream(ps).iterator();
			}
		}, WANDER {
			Iterator<Vector2> genPoints() {
				return new Iterator<Vector2>() {
					public boolean hasNext() {
						return true;
					}
					
					Random r = new Random();
					double angle = r.nextDouble(Math.PI * 2);
					double step = .05, turn_factor = Math.toRadians(20);
					double x = r.nextDouble(-1, 1), y = r.nextDouble(-1, 1);
					double square_size = 3, threshold = .5;

					public Vector2 next() {
						boolean b = Math.abs(x) >= square_size / 2 - threshold ||
									Math.abs(y) >= square_size / 2 - threshold; 

						if (b) {
							angle += turn_factor;
						} else {
							angle += clamp(
								r.nextDouble(-turn_factor * 2, turn_factor * 2),
								-turn_factor, turn_factor
							);
						}
						
						x += Math.cos(angle) * step;
						y += Math.sin(angle) * step;

						// System.out.printf("#%d. (%.2f, %.2f); %f %b\n", i, x, y, Math.toDegrees(angle) % 360, b);
						return getPoss(new Vector2(x, y), 3, Math.toRadians(62.2));
					}
				};
			}
		},

		;

		abstract Iterator<Vector2> genPoints();
	}

	static Vector2[] getPositionsFromFile(String file) throws FileNotFoundException {
		ArrayList<Vector2> l = new ArrayList<Vector2>();
		try (DataInputStream dis = new DataInputStream(new FileInputStream(file))) {
			while (true) {
				double x1 = dis.readDouble(), x2 = dis.readDouble();
				l.add(new Vector2(x1, x2));
			}
		} catch (Exception e) {}
		return l.toArray(new Vector2[l.size()]);
	}

	static double getCamDistance(double square_size, double fov) {
		return 0.5 * square_size * (
			1 / Math.tan(
				0.5 * fov
			) + 1
		);
	}

	static Config getConfig(String file) {
		try (var dis = new DataInputStream(new FileInputStream(file))) {
			int id = dis.readInt();

			String addr = dis.readUTF();
			int port = dis.readInt();

			double x = dis.readDouble();
			double y = dis.readDouble();
			double r = dis.readDouble();
			double f = dis.readDouble();

			long ms = dis.readLong();
			return new Config(id, new InetSocketAddress(addr, port), new Vector2(x, y), r, f, ms);
		} catch (Exception e) {
			e.printStackTrace();
		}
		return null;
	}

	public static void main(String[] args) throws Exception {
		Config config = getConfig(args[0]);
		System.err.println("Starting with config:");
		System.err.println(config);

		PositionGenerator generator = PositionGenerator.WANDER;
		Iterator<Vector2> positions = generator.genPoints();

		try (var ds = new DatagramSocket()) {
			ds.setSoTimeout(1);

			byte[] buff = ByteBuffer.allocate(1 + 4 * 8)
				.put((byte) 0xcc)
				.putDouble(config.pos.x)
				.putDouble(config.pos.y)
				.putDouble(config.rot)
				.putDouble(config.fov)
				.array();
			ds.send(new DatagramPacket(buff, buff.length, config.serverAddress));

			for (int i = 0; positions.hasNext(); i++) {
				byte[] dpBuff = new byte[4];
				var rec = new DatagramPacket(dpBuff, dpBuff.length);
				try {
					ds.receive(rec);
					if (dpBuff[0] == (byte) 0xcd)
						break;
				} catch (SocketTimeoutException e) {}

				Vector2 p = positions.next();

				double d;
				if (config.id == 0)
					d = p.x;
				else if (config.id == 1)
					d = p.y;
				else
					throw new Exception("bruh");
		
				System.err.printf("Sending pos #%d | %.3f\n", i, d);

				buff = ByteBuffer.allocate(8)
					.putDouble(d)
					.array();
				ds.send(new DatagramPacket(buff, buff.length, config.serverAddress));
				Thread.sleep(config.ms);
			}
		}
	}
}
