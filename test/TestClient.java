import java.io.FileNotFoundException;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.FileInputStream;
import java.net.ServerSocket;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.Arrays;
import java.util.Random;
import java.net.Socket;

class Main {
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

	static Vector2[] getPositionsFromFile() throws FileNotFoundException {
		ArrayList<Vector2> l = new ArrayList<Vector2>();
		try (DataInputStream dis = new DataInputStream(new FileInputStream("dump"))) {
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

	public static void main(String[] args) throws Exception {
		if (args.length < 1) {
			System.err.println("No client index provided, exiting...");
			System.exit(1);
		}
		int me = Integer.parseInt(args[0]);
		int port = 12340 + me;
		System.err.printf("Running as %d on port %d", me, port);

		int ms = 50;
		if (args.length >= 2)
			ms = Integer.parseInt(args[1]);

		PositionGenerator generator = PositionGenerator.WANDER;
		Iterator<Vector2> positions = generator.genPoints();

		try (ServerSocket ss = new ServerSocket(port)) {
			ss.setSoTimeout(0);

			Socket s = ss.accept();
			DataOutputStream dos = new DataOutputStream(s.getOutputStream());

			for (int i = 0; positions.hasNext(); i++) {
				Vector2 p = positions.next();

				double d;
				if (me == 0)
					d = p.x;
				else if (me == 1)
					d = p.y;
				else
					throw new Exception("bruh");
		
				System.err.printf("Sending pos #%d | %.3f\n", i, d);
				dos.writeDouble(d);
				Thread.sleep(ms);
			}
		}
	}
}
