import java.io.DataInputStream;
import java.util.Collections;
import java.io.EOFException;
import java.awt.Graphics2D;
import java.util.ArrayList;
import java.util.List;

import javax.swing.JFrame;

import java.awt.BasicStroke;
import java.awt.Canvas;
import java.awt.Color;
import java.awt.Font;

public class TestDisplay {

	public record PlacedCamera(String host, double x, double y, double rot, double fov) {}
	public record Position(double x, double y, double r) {}


	private static final class MyLock {}

	static Position pos;
	static MyLock posLock = new MyLock();
	static List<PlacedCamera> cameras = Collections.synchronizedList(new ArrayList<>());

	public static void main(String[] args) throws Exception {
		new Thread(() -> {
			var bis = new DataInputStream(System.in);
			while (true) {
				try {
					var what = bis.readInt();
					switch (what) {
						case 0:
							var p = new Position(bis.readDouble(), bis.readDouble(), bis.readDouble());
							synchronized (posLock) {
								pos = p;
							}
							break;

						case 1:
							var c = new PlacedCamera(
								bis.readUTF(),
								bis.readDouble(),
								bis.readDouble(),
								bis.readDouble(),
								bis.readDouble()
							);
							synchronized (cameras) {
								cameras.add(c);
							}
							break;

						case 2:
							var h = bis.readUTF();
							cameras.removeIf(cf -> cf.host == h);
							break;

						default: throw new Exception("WHAT??? " + what);
					}
				} catch (EOFException e) {
					break;
				} catch (Exception e) {
					e.printStackTrace();
					return;
				}
			}
		}).start();

		JFrame f = new JFrame("camloc");
		f.setDefaultCloseOperation(JFrame.EXIT_ON_CLOSE);
		f.setSize(800, 800);
		f.setLocationRelativeTo(null);

		Canvas c = new Canvas();
		f.add(c);

		f.setVisible(true);

		renderLoop(c);
	}

	static int cam_size = 20;
	static int dot_size = 6;
	static double square_size = 3;
	static double rectPercent = .35;
	static void render(Graphics2D g, int w, int h) {
		int cx = w / 2, cy = h / 2;

		g.setColor(Color.darkGray);
		g.fillRect(0, 0, w, h);

		g.setColor(Color.yellow);
		g.setStroke(new BasicStroke(3));
		g.drawLine(0, cy, w, cy);
		g.drawLine(cx, 0, cx, h);

		double rw = w * rectPercent;
		double rh = h * rectPercent;

		int ax = cx - (int) Math.round(0.5 * rw);
		int ay = cy - (int) Math.round(0.5 * rh);

		g.drawRect(
			ax, ay,
			(int) Math.round(rw),
			(int) Math.round(rh)
		);

		synchronized (cameras) {
			for (PlacedCamera c : cameras) {
				int xx = cx + (int) Math.round(c.x / square_size * rw);
				int yy = cy - (int) Math.round(c.y / square_size * rh);

				g.setColor(Color.green);
				g.drawRect(xx - cam_size / 2, yy - cam_size / 2, cam_size, cam_size);

				g.drawLine(
					xx, yy,
					cx + (int) Math.round((c.x + Math.cos(c.rot + c.fov / 2) * 10) / square_size * rw),
					cy - (int) Math.round((c.y + Math.sin(c.rot + c.fov / 2) * 10) / square_size * rh)
				);
				g.drawLine(
					xx, yy,
					cx + (int) Math.round((c.x + Math.cos(c.rot - c.fov / 2) * 10) / square_size * rw),
					cy - (int) Math.round((c.y + Math.sin(c.rot - c.fov / 2) * 10) / square_size * rh)
				);

				g.setFont(new Font("sans", Font.PLAIN, 15));
				g.drawString(c.host, xx + cam_size, yy);
			}
		}

		synchronized (posLock) {
			if (pos != null) {

				g.setColor(Color.yellow);
				for (PlacedCamera c : cameras) {
					int cam_x = cx + (int) Math.round(c.x / square_size * rw);
					int cam_y = cy - (int) Math.round(c.y / square_size * rh);
	
					int p_x = cx + (int) Math.round(pos.x / square_size * rw);
					int p_y = cy - (int) Math.round(pos.y / square_size * rh);
	
					g.drawLine(cam_x, cam_y, p_x, p_y);
				}

				int x = cx + (int) Math.round(pos.x / square_size * rw);
				int y = cy - (int) Math.round(pos.y / square_size * rh);

				g.setColor(Color.red);
				if (Double.isNaN(pos.r))
					g.fillOval(x - dot_size / 2, y - dot_size / 2, dot_size, dot_size);
				else
					g.fillPolygon(
						new int[] { x, x + dot_size / 2, x, x - dot_size / 2 },
						new int[] { y - dot_size / 2, y + dot_size / 2, y, y + dot_size / 2 },
						4
					);
			}
		}
	}

	static int pointsDrawn = 0, camerasDrawn = 0;
	static void renderLoop(Canvas c) {
		while (true) {
			var bs = c.getBufferStrategy();
			if (bs == null) {
				c.createBufferStrategy(2);
				continue;
			}

			var g = (Graphics2D) bs.getDrawGraphics();

			render(g, c.getWidth(), c.getHeight());

			g.dispose();
			bs.show();

			try {
				Thread.sleep(30);
			} catch (InterruptedException e) {
				e.printStackTrace();
			}
		}
	}
}
