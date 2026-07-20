NAME=av_log_intercept
gcc -Wall -Wextra -O2 -g -shared -fPIC \
-o $NAME.so $NAME.c