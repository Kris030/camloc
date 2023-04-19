base_dir="$HOME/build/opencv"

export OPENCV_LINK_LIBS=opencv_highgui,opencv_objdetect,opencv_dnn,opencv_videostab,opencv_calib3d,opencv_features2d,opencv_stitching,opencv_flann,opencv_videoio,opencv_rgbd,opencv_aruco,opencv_video,opencv_ml,opencv_imgcodecs,opencv_imgproc,opencv_core,ittnotify,tbb,liblibwebp,liblibtiff,liblibjpeg-turbo,liblibpng,liblibopenjp2,ippiw,ippicv,liblibprotobuf,quirc,zlib
export OPENCV_LINK_PATHS=$base_dir/lib,$base_dir/lib/opencv4/3rdparty,/usr/lib/x86_64-linux-gnu
export OPENCV_INCLUDE_PATHS=$base_dir/include/opencv4

cargo build -vv --release
