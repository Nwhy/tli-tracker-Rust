# Screenshots

Place the example screenshots (or your real exported screenshots) in the `images/` folder and reference them from the README or project page.

Example usage in README:

![Overlay screenshot](images/overlay_screenshot.png)
![Web interface screenshot](images/webinterface_screenshot.png)

If you keep the SVG files you can also reference them directly:

![Overlay screenshot SVG](images/overlay_screenshot.svg)
![Web interface screenshot SVG](images/webinterface_screenshot.svg)

Convert SVG to PNG (optional) on CachyOS:

- Using librsvg:
  sudo pacman -S librsvg
  rsvg-convert -w 1200 -h 675 images/overlay_screenshot.svg -o images/overlay_screenshot.png

- Using ImageMagick:
  sudo pacman -S imagemagick
  convert -density 150 images/webinterface_screenshot.svg images/webinterface_screenshot.png
