FROM ubuntu

RUN apt-get update && apt-get install -y curl sqlite3
RUN curl -L -O https://github.com/lunatic-solutions/lunatic/releases/download/v0.13.2/lunatic-linux-amd64.tar.gz
RUN tar -xzf lunatic-linux-amd64.tar.gz

ENTRYPOINT [ "/lunatic" ]