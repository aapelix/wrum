#pragma once
#include "transport.h"
Transport *transport_enet_host(uint16_t port, int max_peers);
Transport *transport_enet_connect(const char *hostname, uint16_t port);
