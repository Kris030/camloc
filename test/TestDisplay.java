import java.io.DataInputStream;
import java.util.Collections;
import java.awt.BasicStroke;
import java.io.EOFException;
import java.io.IOException;
import java.awt.Graphics2D;
import java.util.ArrayList;
import java.util.List;

import javax.swing.JFrame;
import java.awt.Canvas;
import java.awt.Color;
import java.awt.Point;
import java.awt.Font;

public class TestDisplay {

	static List<Point.Double> points = Collections.synchronizedList(new ArrayList<>());

	public static void main(String[] args) throws Exception {
		new Thread(() -> {
			var bis = new DataInputStream(System.in);
			while (true) {
				try {
					var p = new Point.Double(bis.readDouble(), bis.readDouble());
					synchronized (points) {
						points.add(p);
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

	static void renderLoop(Canvas c) {
		c.createBufferStrategy(2);
		var bs = c.getBufferStrategy();

		while (true) {
			var g = (Graphics2D) bs.getDrawGraphics();

			int w = c.getWidth(), h = c.getHeight();
			int cx = w / 2, cy = h / 2;

			g.setColor(Color.darkGray);
			g.fillRect(0, 0, w, h);

			g.setColor(Color.yellow);
			g.setStroke(new BasicStroke(3));
			g.drawLine(0, cy, w, cy);
			g.drawLine(cx, 0, cx, h);

			double rectPercent = .85;
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
					double square_size = 3;
		
					int xx = cx + (int) Math.round(p.x / square_size * rw);
					int yy = cy - (int) Math.round(p.y / square_size * rh);
		
					g.setColor(Color.red);
					g.fillOval(xx - 3, yy - 3, 6, 6);
				}
			}

			g.setColor(new Color(120, 180, 0));
			g.setFont(new Font("Comic", Font.BOLD, 40));
			g.drawString(Integer.toString(points.size()), w - 150, h - 20);
			
			g.dispose();
			bs.show();

			try {
				Thread.sleep(15);
			} catch (InterruptedException e) {
				e.printStackTrace();
			}
		}
	}
}
