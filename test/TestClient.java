import java.io.FileNotFoundException;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.FileInputStream;
import java.net.ServerSocket;
import java.util.ArrayList;
import java.net.Socket;

import java.awt.Point;

class Main {

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

	static Point.Double getPoss(Point.Double p, double square_size, double fov) {
		double cd = 0.5 * square_size * (
			1 / Math.tan(
				0.5 * fov
			) + 1
		);

		double m1 = Math.atan2(p.y, p.x + cd);
		double x1 = 1 - (m1 + fov / 2) / fov;
	
		double m2 = Math.atan(p.x / (p.y - cd));
		double x2 = 1 - (m2 + fov / 2) / fov;

		return new Point.Double(x1, x2);
	}

	static Point.Double[] genPositions() {
		Point.Double[] ps = new Point.Double[7];
		int a = 1;
		for (int i = 0; i < 7 * a; i++) {
			double x = 0.2 / a * i;
			double y = Math.sqrt(x) / 3;
			ps[i] = getPoss(new Point.Double(x, y), 3, Math.toRadians(62.2));
		}

		return ps;
	}

	public static void main(String[] args) throws Exception {
		int me = Integer.parseInt(args[0]);
		int port = 12340 + me;
		System.err.println("Running as " + me + " on port " + port);

		Point.Double[] positions = genPositions();

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
				Thread.sleep(100);
			}
		}
	}
}
