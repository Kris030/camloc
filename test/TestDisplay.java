import java.io.DataInputStream;
import java.util.Collections;
import java.awt.BasicStroke;
import java.io.EOFException;
import java.io.IOException;
import java.awt.Graphics2D;
import java.util.ArrayList;
import java.util.List;

import java.awt.image.BufferedImage;
import javax.swing.JFrame;
import java.awt.Canvas;
import java.awt.Color;
import java.awt.Point;
import java.awt.Font;

public class TestDisplay {

	public record PlacedCamera(String host, double x, double y, double rot, double fov) {}

	static List<Point.Double> points = Collections.synchronizedList(new ArrayList<>());
	static List<PlacedCamera> cameras = Collections.synchronizedList(new ArrayList<>());

	public static void main(String[] args) throws Exception {
		new Thread(() -> {
			var bis = new DataInputStream(System.in);
			while (true) {
				try {
					var what = bis.readInt();
					switch (what) {
						case 0:
							var p = new Point.Double(bis.readDouble(), bis.readDouble());
							synchronized (points) {
								points.add(p);
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

						default: throw new RuntimeException("WHAT??? " + what);
					}
				} catch (EOFException e) {
					break;
				} catch (IOException e) {
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
	static float col_t = .01f, hue;
	static double square_size = 3;
	static double rectPercent = .35;
	static BufferedImage redraw(int w, int h) {
		BufferedImage img = new BufferedImage(w, h, BufferedImage.TYPE_3BYTE_BGR);
		Graphics2D g = img.createGraphics();

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

		synchronized (points) {
			for (var p : points) {
				int xx = cx + (int) Math.round(p.x / square_size * rw);
				int yy = cy - (int) Math.round(p.y / square_size * rh);

				g.setColor(Color.getHSBColor(hue, 0.5f, 1f));
				hue += col_t;
				if (hue >= 360)
					hue -= 360;
				g.fillRect(xx - dot_size / 2, yy - dot_size / 2, dot_size, dot_size);
			}
			pointsDrawn = points.size();
		}

		g.dispose();

		return img;
	}

	static void update(BufferedImage img) {
		Graphics2D g = img.createGraphics();

		int w = img.getWidth(), h = img.getHeight();
		int cx = w / 2, cy = h / 2;

		double rw = w * rectPercent;
		double rh = h * rectPercent;

		synchronized (points) {
			for (int i = pointsDrawn; i < points.size(); i++) {
				var p = points.get(i);

				int xx = cx + (int) Math.round(p.x / square_size * rw);
				int yy = cy - (int) Math.round(p.y / square_size * rh);

				g.setColor(Color.getHSBColor(hue, 0.5f, 1f));
				hue += col_t;
				if (hue >= 360)
					hue -= 360;
				g.fillRect(xx - dot_size / 2, yy - dot_size / 2, dot_size, dot_size);

			}

			synchronized (cameras) {
				for (int i = camerasDrawn; i < cameras.size(); i++) {
					var c = cameras.get(i);

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

			pointsDrawn = points.size();
		}

		g.dispose();
	}

	static int pointsDrawn = 0, camerasDrawn = 0;
	static void renderLoop(Canvas c) {
		BufferedImage imgcache = redraw(c.getWidth(), c.getHeight());

		while (true) {
			var bs = c.getBufferStrategy();
			if (bs == null) {
				c.createBufferStrategy(2);
				continue;
			}

			var g = (Graphics2D) bs.getDrawGraphics();

			int w = c.getWidth(), h = c.getHeight();
			int cx = w / 2, cy = h / 2;

			double rw = w * rectPercent;
			double rh = h * rectPercent;

			if (imgcache.getWidth() != w || imgcache.getHeight() != h)
				imgcache = redraw(w, h);
			else
				update(imgcache);

			g.drawImage(imgcache, 0, 0, null);

			Point.Double lastPoint = null;
			synchronized (points) {
				if (!points.isEmpty())
					lastPoint = points.get(points.size() - 1);
			}

			if (lastPoint != null) {
				g.setColor(Color.yellow);
				for (PlacedCamera cam : cameras) {
					int cam_x = cx + (int) Math.round(cam.x / square_size * rw);
					int cam_y = cy - (int) Math.round(cam.y / square_size * rh);

					int p_x = cx + (int) Math.round(lastPoint.x / square_size * rw);
					int p_y = cy - (int) Math.round(lastPoint.y / square_size * rh);

					g.drawLine(cam_x, cam_y, p_x, p_y);
				}
			}

			g.setColor(new Color(120, 180, 0));
			g.setFont(new Font("Comic", Font.BOLD, 40));
			g.drawString(Integer.toString(points.size()), w - 150, h - 20);

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
