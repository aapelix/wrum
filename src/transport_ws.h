#pragma once
#include "transport.h"
Transport *transport_ws_connect(const char *hostname, uint16_t port);
