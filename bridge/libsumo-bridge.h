#include <stddef.h>

#include <cstddef>

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
struct SimulationStatistics {
    size_t current_time_millis;
    size_t active_vehicle_count;
    size_t loaded_vehicle_count;
    size_t departed_vehicle_count;
    size_t arrived_vehicle_count;
    size_t starting_teleport_count;
    size_t ending_teleport_count;
    size_t collision_count;
    size_t min_expected_vehicle_count;
};
struct LaneTrafficMetrics {
    size_t vehicle_count;
    size_t queued_vehicle_count;
    double mean_speed;
    double occupancy_percent;
    double waiting_time_seconds;
};
struct Position {
    double x;
    double y;
};
struct Shape {
    struct Position* points;
    size_t len;
};
struct Vehicle {
    char* id;
    char* lane_id;
    double lane_position;
    double speed;
    double acceleration;
    double waiting_time;
    double time_loss;
    char* next_trafficlight;
    size_t next_link_index;
    struct Position position;
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
    struct Shape shape;
};
struct Lane {
    char* id;
    char* edge_id;
    double length;
    struct Shape shape;
};
struct TrafficLight {
    char* id;
};
struct Link {
    char* from_lane;
    char* via_lane;
    char* to_lane;
    char* trafficlight;
    size_t link_index;
};
struct Phase {
    char* trafficlight;
    char* state;
    size_t program_index;
    size_t phase_index;
    size_t* next_indexs;
    size_t next_count;
};
struct Payload sumo_load(const char* cfg, const size_t threads,
                         const char* statics, const char* tripinfo,
                         const char* edgedata);
struct Payload sumo_step();
struct Payload sumo_close();
struct Payload get_parameter(char* obj, char* para);
struct Payload get_option(char* opt);
struct Payload set_trafficlight(const char* trafficlight_id, const char* state);
struct Payload get_trafficlight_state(const char* trafficlight_id);
struct Payload get_lane_traffic_metrics(const char* lane_id);
struct Payload get_vehicles();
struct Payload get_edges();
struct Payload get_junctions();
struct Payload get_lanes();
struct Payload get_trafficlights();
struct Payload get_links();
struct Payload get_phases();
struct Payload get_simulation_statistics();
#ifdef __cplusplus
}
#endif
