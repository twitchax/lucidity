#!bin/bash

export FLY_UDP_IP=$(ip a show eth0 secondary | grep -oP 'inet \K[\d.]+')

/lunatic node --bind-socket $FLY_UDP_IP:3031 $1