LD_PRELOAD=./av_log_intercept.so \
 ffmpeg -i track_video.mp4 -i track_audio.mp4 \
 -stats_period 0.1 \
 -c copy -map 0:v:0 -map 1:a:0 -y output.mp4
