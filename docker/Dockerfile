FROM ubuntu

RUN apt-get update && apt-get install -y curl sqlite3 iproute2

RUN curl -L -O https://github.com/lunatic-solutions/lunatic/releases/download/v0.13.2/lunatic-linux-amd64.tar.gz
RUN tar -xzf lunatic-linux-amd64.tar.gz
#COPY ./docker/lunatic /lunatic

COPY ./docker/app.entrypoint.sh .
RUN chmod a+x app.entrypoint.sh

ENTRYPOINT [ "/app.entrypoint.sh" ]