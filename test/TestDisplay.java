import java.io.DataInputStream;
import java.util.Collections;
import java.io.EOFException;
import java.awt.Graphics2D;
import java.awt.Point;
import java.awt.RenderingHints;
import java.awt.event.MouseAdapter;
import java.awt.event.MouseEvent;
import java.awt.event.MouseWheelEvent;
import java.awt.geom.AffineTransform;
import java.awt.geom.Ellipse2D;
import java.awt.geom.Line2D;
import java.awt.geom.Path2D;
import java.awt.geom.Point2D;
import java.awt.geom.Rectangle2D;
import java.awt.image.BufferStrategy;
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

	static final double FPS = 60;
	static final double WAIT_MS = 1000 / FPS;

	static final double ZOOM_SENSITIVITY = 0.02, DRAG_SENSITIVITY = 0.001;
	static final double CAMERA_SIZE = 0.075, DOT_SIZE = 0.05;
	static final double IRL_SCALING = .5;

	static double camX, camY, zoomScale = 1;
	static boolean redraw = true;

	public static void main(String[] args) throws Exception {
		startDataThread();

		Canvas c = initGUI();
		renderLoop(c);
	}

	private static void startDataThread() {
		new Thread(() -> {
			DataInputStream bis = new DataInputStream(System.in);
			while (true) {
				try {
					int what = bis.readInt();
					switch (what) {
						case 0:
							Position p = new Position(bis.readDouble(), bis.readDouble(), bis.readDouble());
							synchronized (posLock) {
								pos = p;
							}
							redraw = true;
							break;

						case 1:
							PlacedCamera c = new PlacedCamera(
								bis.readUTF(),
								bis.readDouble(),
								bis.readDouble(),
								bis.readDouble(),
								bis.readDouble()
							);
							synchronized (cameras) {
								cameras.add(c);
							}
							redraw = true;
							break;

						case 2:
							String h = bis.readUTF();
							cameras.removeIf(cf -> cf.host == h);
							redraw = true;
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
	}

	private static Canvas initGUI() {
		JFrame f = new JFrame("camloc");
		f.setDefaultCloseOperation(JFrame.EXIT_ON_CLOSE);
		f.setSize(800, 800);
		f.setLocationRelativeTo(null);

		Canvas c = new Canvas();
		f.add(c);

		f.setVisible(true);

		MouseAdapter m = new MouseAdapter() {
			boolean leftClicking = false;
			Point lastDrag;

			public void mouseWheelMoved(MouseWheelEvent e) {
				Point2D.Double p = toWorldSpace(e.getPoint(), c.getWidth(), c.getHeight());

				double rot = e.getPreciseWheelRotation();
				double sign = Math.signum(rot);
				
				double modifier = ZOOM_SENSITIVITY;
				if ((e.getModifiersEx() & MouseEvent.ALT_DOWN_MASK) == MouseEvent.ALT_DOWN_MASK)
					modifier *= 5;

				camX += p.x * modifier * sign;
				camY += p.y * modifier * sign;
				
				zoomScale -= rot * modifier;

				redraw = true;
			}

			public void mouseDragged(MouseEvent e) {
				if (!leftClicking)
					return;

				Point p = e.getPoint();
				
				camX += (p.x - lastDrag.x) * zoomScale * DRAG_SENSITIVITY;
				camY -= (p.y - lastDrag.y) * zoomScale * DRAG_SENSITIVITY;

				lastDrag = p;
				redraw = true;
			}
			public void mousePressed(MouseEvent e) {
				if (e.getButton() == MouseEvent.BUTTON1) {
					lastDrag = e.getPoint();
					leftClicking = true;
					redraw = true;
				}
			}
			public void mouseReleased(MouseEvent e) {
				if (e.getButton() == MouseEvent.BUTTON1) {
					leftClicking = false;
					lastDrag = null;
					redraw = true;
				}
			}
		};

		c.addMouseMotionListener(m);
		c.addMouseWheelListener(m);
		c.addMouseListener(m);

		return c;
	}

	static Point2D.Double toWorldSpace(java.awt.Point p, int w, int h) {
		return new Point2D.Double(
			(p.x + w / 2d) / w - 1,
			(h - p.y + h / 2d) / h - 1
		);
	}
	
	static void render(Graphics2D g, int canvasW, int canvasH) {
		setRenderHints(g);

		g.setColor(Color.darkGray);
		g.fillRect(0, 0, canvasW, canvasH);

		// flip y axis
		g.scale(1, -1);

		// recenter
		g.translate(canvasW / 2d, canvasH / 2d - canvasH);

		// make screen 1x1
		double sreenScale = Math.min(canvasW, canvasH);
		g.scale(sreenScale, sreenScale);

		// move to camera position
		g.translate(camX, camY);

		// apply zoom + scale to real life
		double scale = zoomScale * IRL_SCALING;
		g.scale(scale, scale);
		
		g.setStroke(new BasicStroke(0.005f));
		g.setColor(Color.gray);
		g.fill(new Ellipse2D.Double(-DOT_SIZE / 2, -DOT_SIZE / 2, DOT_SIZE, DOT_SIZE));

		synchronized (cameras) {
			for (PlacedCamera c : cameras) {
				g.setColor(Color.green);
				g.draw(new Rectangle2D.Double(
					c.x - CAMERA_SIZE / 2,
					c.y - CAMERA_SIZE / 2,
					CAMERA_SIZE,
					CAMERA_SIZE
				));

				g.draw(new Line2D.Double(
					c.x, c.y,
					(c.x + Math.cos(c.rot + c.fov / 2) * 10),
					(c.y + Math.sin(c.rot + c.fov / 2) * 10)
				));
				g.draw(new Line2D.Double(
					c.x, c.y,
					(c.x + Math.cos(c.rot - c.fov / 2) * 10),
					(c.y + Math.sin(c.rot - c.fov / 2) * 10)
				));

				g.setColor(Color.white);
				g.setFont(new Font("sans", Font.PLAIN, 1));
				
				AffineTransform at = g.getTransform();
				g.translate(c.x + CAMERA_SIZE, c.y);
				g.scale(0.05, -0.05);
				g.drawString(c.host, 0, 0);

				g.setTransform(at);
			}
		}

		synchronized (posLock) {
			if (pos != null) {

				g.setColor(Color.yellow);
				for (PlacedCamera c : cameras)
					g.draw(new Line2D.Double(
						c.x, c.y,
						pos.x, pos.y
					));

				double x = pos.x;
				double y = pos.y;

				g.setColor(Color.red);
				if (Double.isNaN(pos.r))
					g.fill(new Ellipse2D.Double(x - DOT_SIZE / 2, y - DOT_SIZE / 2, DOT_SIZE, DOT_SIZE));
				else {
					Path2D.Double p = new Path2D.Double();
					
					p.moveTo(x, y + DOT_SIZE / 2);
					p.moveTo(x + DOT_SIZE / 2, y - DOT_SIZE / 2);
					p.moveTo(x, y);
					p.moveTo(x - DOT_SIZE / 2, y - DOT_SIZE / 2);
					p.closePath();

					g.fill(p);
				}
			}
		}
	}

	static void renderLoop(Canvas c) {
		while (true) {
			long start = System.nanoTime();

			BufferStrategy bs = c.getBufferStrategy();
			if (bs == null) {
				c.createBufferStrategy(2);
				continue;
			}

			Graphics2D g = (Graphics2D) bs.getDrawGraphics();

			if (redraw) {
				redraw = false;
				render(g, c.getWidth(), c.getHeight());
			}

			g.dispose();
			bs.show();
			
			long realWait = Math.round(WAIT_MS - (System.nanoTime() - start) / 1_000_000d);

			if (realWait > 0) {
				try {
					Thread.sleep(realWait);
				} catch (InterruptedException e) {
					e.printStackTrace();
				}
			}
		}
	}

	private static void setRenderHints(Graphics2D g) {
		g.setRenderingHint(RenderingHints.KEY_ALPHA_INTERPOLATION, RenderingHints.VALUE_ALPHA_INTERPOLATION_QUALITY);
        g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_QUALITY);
	}
}
