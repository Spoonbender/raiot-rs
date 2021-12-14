MQTT
====
CONNECT -> CONNACK
PUBLISH -> PUBACK
SUBSCRIBE -> SUBACK
UNSUBSCRIBE -> UNSUBACK
PINGREQ -> PINGRESP
DISCONNECT

IOT FLOWS
=========

TWIN:
SUB -> [SUBACK] -> PUBLISH / PUBACK ? -> UNSUB -> UNSUBACK

C2D:
SUB -> [SUBACK] -> PUBLISH / PUBACK ? -> UNSUB -> UNSUBACK

METHODS:
SUB -> [SUBACK] -> PUBLISH / PUBACK ? -> UNSUB -> UNSUBACK

TELEMETRY:
PUBLISH / PUBACK

IN-FLIGHT:
PUB, SUB, PING, CONN?

CLIENT
Blocking
Callbacks
Thread Pool
Dedicated thread(s)

Mqtt socket
Mqtt session
Iot socket?
Iot session?

LAYERS
======
Transport

Session
- Specific device/module
- In charge of packet IDs
- Bookkeeping packets for req/res pattern

Client
- Holds session internally
- Optional reconnection mechanism