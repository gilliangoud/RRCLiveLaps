FROM balenablocks/browser

COPY src /usr/src/app/public

ENV LAUNCH_URL=file:///usr/src/app/public/index.html
ENV KIOSK=1

