#include <stddef.h>

#define BRIDGE_SUCCESS 0
#define BRIDGE_SUMO_EXCEPTION 1
#define BRIDGE_MALLOC_FAILED 2

#ifdef __cplusplus
extern "C" {
#endif
struct Payload {
    size_t status;
    void* payload;
    size_t len;
};
struct Vehicle {
    char* id;
    char* edge_id;
    size_t lane_index;
};
struct Edge {
    char* id;
    char* name;
    char* from;
    char* to;
    size_t lane_number;
};
struct Junction {
    char* id;
};
struct Lane {
    char* id;
    char* edge_id;
    double length;
};
struct Payload sumo_load(const char* cfg);
struct Payload sumo_step();
struct Payload sumo_close();
struct Payload get_parameter(char* obj, char* para);
struct Payload get_option(char* opt);
struct Payload get_vehicles();
struct Payload get_edges();
struct Payload get_junctions();
struct Payload get_lanes();
#ifdef __cplusplus
}
#endif
