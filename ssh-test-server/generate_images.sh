#!/bin/sh
# based on something ChatGPT created
# generate_images.sh

# Create the images directory
mkdir -p /home/test/images

# Generate PNG8 images displaying numbers using ImageMagick with DejaVu-Sans-Bold font
for i in $(seq 1 5); do
    convert -size 100x100 xc:lightblue -font DejaVu-Sans-Bold -background none -gravity center -pointsize 40 -depth 8 -draw "text 0,0 '$i'" /home/test/images/image$i.png
done

