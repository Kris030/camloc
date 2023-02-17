import java.io.FileNotFoundException;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.FileInputStream;
import java.net.ServerSocket;
import java.util.ArrayList;
import java.util.Random;
import java.net.Socket;

import java.awt.Point;

class Main {

	static double clamp(double value, double min, double max) {
		if (value >= max)
			return max;
		if (value <= min)
			return min;
		return value;
	}

	static Point.Double getPoss(Point.Double p, double square_size, double fov) {
		double cd = getCamDistance(square_size, fov);

		double m1 = Math.atan2(p.y, p.x + cd);
		double x1 = 1 - (m1 + fov / 2) / fov;
	
		double m2 = Math.atan(p.x / (p.y - cd));
		double x2 = 1 - (m2 + fov / 2) / fov;

		return new Point.Double(x1, x2);
	}

	static enum PositionGenerator {

		SQRT {
			Point.Double[] genPoints() {
				Point.Double[] ps = new Point.Double[7];
				int a = 1;
				for (int i = 0; i < 7 * a; i++) {
					double x = 0.2 / a * i;
					double y = Math.sqrt(x) / 3;
					ps[i] = getPoss(new Point.Double(x, y), 3, Math.toRadians(62.2));
				}

				return ps;
			}
		}, WANDER {
			Point.Double[] genPoints() {
				Random r = new Random();
				Point.Double[] ps = new Point.Double[500];

				double angle = r.nextDouble(Math.PI * 2);
				double step = .05, turn_factor = Math.toRadians(20);
				double x = 0, y = 0;
				
				double square_size = 3, threshold = .5;

				for (int i = 0; i < ps.length; i++) {
					boolean b = Math.abs(x) >= square_size / 2 - threshold ||
								Math.abs(y) >= square_size / 2 - threshold; 
					if (b)
						angle += turn_factor;
					else
						angle += clamp(
							r.nextDouble(-turn_factor * 2, turn_factor * 2),
							-turn_factor, turn_factor
						);
					
					x += Math.cos(angle) * step;
					y += Math.sin(angle) * step;

					// System.out.printf("#%d. (%.2f, %.2f); %f %b\n", i, x, y, Math.toDegrees(angle) % 360, b);
					ps[i] = getPoss(new Point.Double(x, y), 3, Math.toRadians(62.2));
				}

				return ps;
			}
		},

		;

		abstract Point.Double[] genPoints();
	}

	static Point.Double[] getPositionsFromFile() throws FileNotFoundException {
		ArrayList<Point.Double> l = new ArrayList<Point.Double>();
		try (DataInputStream dis = new DataInputStream(new FileInputStream("dump"))) {
			while (true) {
				double x1 = dis.readDouble(), x2 = dis.readDouble();
				l.add(new Point.Double(x1, x2));
			}
		} catch (Exception e) {}
		return l.toArray(new Point.Double[l.size()]);
	}

	static double getCamDistance(double square_size, double fov) {
		return 0.5 * square_size * (
			1 / Math.tan(
				0.5 * fov
			) + 1
		);
	}

	public static void main(String[] args) throws Exception {
		int me = Integer.parseInt(args[0]);
		int port = 12340 + me;
		System.err.println("Running as " + me + " on port " + port);

		Point.Double[] positions = PositionGenerator.WANDER.genPoints();

		try (ServerSocket ss = new ServerSocket(port)) {
			ss.setSoTimeout(0);

			Socket s = ss.accept();
			DataOutputStream dos = new DataOutputStream(s.getOutputStream());

			for (int i = 0; i < positions.length; i++) {
				double d;
				if (me == 0)
					d = positions[i].x;
				else if (me == 1)
					d = positions[i].y;
				else
					throw new Exception("bruh");
		
				System.err.println("Sending pos #" + i + " | " + d);
				dos.writeDouble(d);
				Thread.sleep(500);
			}
		}
	}
}
