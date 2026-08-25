#include "libsumo-bridge.h"

#include <cxxabi.h>
#include <libsumo/libsumo.h>

#include <cstddef>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "libsumo/Edge.h"
#include "libsumo/Junction.h"
#include "libsumo/Lane.h"
#include "libsumo/Simulation.h"
#include "libsumo/TrafficLight.h"
#include "libsumo/Vehicle.h"

const char* demangle(const char* name) {
    int status = 0;

    const char* p = abi::__cxa_demangle(name, nullptr, nullptr, &status);

    if (status) {
        char* s = strdup("<DEMANGLE FAILED>");
        if (!s) s = (char*)BRIDGE_MALLOC_FAILED;
        return s;
    } else {
        return p;
    }
}

inline Payload report_exception(const std::exception& e) {
    const char* damangled = demangle(typeid(e).name());
    if ((size_t)damangled == BRIDGE_MALLOC_FAILED)
        return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
    else
        return Payload{BRIDGE_SUMO_EXCEPTION, (void*)damangled, 0};
}

Payload sumo_load(const char* cfg) {
    try {
        libsumo::Simulation::load({"-c", cfg, "--threads", "2"});
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, 0, 0};
}

Payload sumo_step() {
    try {
        libsumo::Simulation::step();
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, 0, 0};
}

Payload sumo_close() {
    try {
        libsumo::Simulation::close();
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, 0, 0};
}

Payload get_parameter(char* obj, char* para) {
    const char* result = NULL;
    try {
        std::string s = libsumo::Simulation::getParameter(obj, para);
        result = strdup(s.c_str());
        if (result == NULL) {
            return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
        }
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, (void*)result, 0};
}

Payload get_option(char* opt) {
    const char* result = NULL;
    try {
        std::string s = libsumo::Simulation::getOption(opt);
        result = strdup(s.c_str());
        if (result == NULL) {
            return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
        }
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, (void*)result, 0};
}

Payload get_vehicles() {
    Vehicle* vehicles = NULL;
    size_t size = 0;
    try {
        auto vehicles_vec = libsumo::Vehicle::getIDList();
        if (vehicles_vec.size() == 0) {
            return Payload{BRIDGE_SUCCESS, 0, 0};
        }
        vehicles = (Vehicle*)malloc(vehicles_vec.size() * sizeof(Vehicle));
        if (vehicles == NULL) return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
        size = vehicles_vec.size();
        for (size_t i = 0; i < size; i++) {
            vehicles[i].id = strdup(vehicles_vec[i].c_str());
            if (vehicles[i].id == NULL) {
                for (size_t j = 0; j < i; j++) free(vehicles[j].id);
                free(vehicles);
                return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
            }
            vehicles[i].edge_id = NULL;
            vehicles[i].lane_index = 0;
        }
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, (void*)vehicles, size};
}

Payload get_edges() {
    Edge* edges = NULL;
    size_t size = 0;
    try {
        auto edges_vec = libsumo::Edge::getIDList();
        if (edges_vec.size() == 0) {
            return Payload{BRIDGE_SUCCESS, 0, 0};
        }
        edges = (Edge*)malloc(edges_vec.size() * sizeof(Edge));
        if (edges == NULL) return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
        size = edges_vec.size();
        for (size_t i = 0; i < size; i++) {
            std::string name = libsumo::Edge::getStreetName(edges_vec[i]);
            std::string from =
                libsumo::Edge::getFromJunction(edges_vec[i].c_str());
            std::string to = libsumo::Edge::getToJunction(edges_vec[i].c_str());
            int lane_number =
                libsumo::Edge::getLaneNumber(edges_vec[i].c_str());
            edges[i].id = strdup(edges_vec[i].c_str());
            edges[i].name = strdup(name.c_str());
            edges[i].from = strdup(from.c_str());
            edges[i].to = strdup(to.c_str());
            edges[i].lane_number = lane_number;
            if (edges[i].id == NULL || edges[i].name == NULL) {
                for (size_t j = 0; j < i; j++) {
                    free(edges[j].id);
                    free(edges[j].name);
                    free(edges[j].from);
                    free(edges[j].to);
                }
                free(edges);
                return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
            }
        }
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, (void*)edges, size};
}

Payload get_junctions() {
    Junction* junctions = NULL;
    size_t size = 0;
    try {
        auto junctions_vec = libsumo::Junction::getIDList();
        if (junctions_vec.size() == 0) {
            return Payload{BRIDGE_SUCCESS, 0, 0};
        }
        junctions = (Junction*)malloc(junctions_vec.size() * sizeof(Junction));
        if (junctions == NULL) return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
        size = junctions_vec.size();
        for (size_t i = 0; i < size; i++) {
            junctions[i].id = strdup(junctions_vec[i].c_str());
            if (junctions[i].id == NULL) {
                for (size_t j = 0; j < i; j++) free(junctions[j].id);
                free(junctions);
                return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
            }
        }
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, (void*)junctions, size};
}

Payload get_lanes() {
    Lane* lanes = NULL;
    size_t size = 0;
    try {
        auto lanes_vec = libsumo::Lane::getIDList();
        if (lanes_vec.size() == 0) {
            return Payload{BRIDGE_SUCCESS, 0, 0};
        }
        lanes = (Lane*)malloc(lanes_vec.size() * sizeof(Lane));
        if (lanes == NULL) return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
        size = lanes_vec.size();
        for (size_t i = 0; i < size; i++) {
            std::string edge_id =
                libsumo::Lane::getEdgeID(lanes_vec[i].c_str());
            double length = libsumo::Lane::getLength(lanes_vec[i].c_str());
            lanes[i].id = strdup(lanes_vec[i].c_str());
            lanes[i].edge_id = strdup(edge_id.c_str());
            lanes[i].length = length;
            if (lanes[i].id == NULL || lanes[i].edge_id == NULL) {
                for (size_t j = 0; j < i; j++) {
                    free(lanes[j].id);
                    free(lanes[j].edge_id);
                }
                free(lanes);
                return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
            }
        }
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, (void*)lanes, size};
}

Payload get_trafficlights() {
    TrafficLight* trafficlights = NULL;
    size_t size = 0;
    try {
        auto trafficlights_vec = libsumo::TrafficLight::getIDList();
        if (trafficlights_vec.size() == 0) {
            return Payload{BRIDGE_SUCCESS, 0, 0};
        }
        trafficlights = (TrafficLight*)malloc(trafficlights_vec.size() *
                                              sizeof(TrafficLight));
        if (trafficlights == NULL) return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
        size = trafficlights_vec.size();
        for (size_t i = 0; i < size; i++) {
            trafficlights[i].id = strdup(trafficlights_vec[i].c_str());
            if (trafficlights[i].id == NULL) {
                for (size_t j = 0; j < i; j++) free(trafficlights[j].id);
                free(trafficlights);
                return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
            }
        }
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    return Payload{BRIDGE_SUCCESS, (void*)trafficlights, size};
}

Payload get_links() {
    Link* links = NULL;
    size_t size = 0;
    std::vector<std::string> from_lanes;
    std::vector<std::string> via_lanes;
    std::vector<std::string> to_lanes;
    std::vector<std::string> link_trafficlights;
    std::vector<size_t> link_indexs;
    try {
        auto lanes = libsumo::Lane::getIDList();
        for (auto id : lanes) {
            auto lane_links = libsumo::Lane::getLinks(id);
            for (auto link : lane_links) {
                size++;
                from_lanes.emplace_back(id);
                via_lanes.emplace_back(link.approachedInternal);
                to_lanes.emplace_back(link.approachedLane);
                link_trafficlights.emplace_back(std::string(""));
                link_indexs.emplace_back(0);
            }
        }
        auto trafficlights = libsumo::TrafficLight::getIDList();
        for (auto id : trafficlights) {
            auto controled_links =
                libsumo::TrafficLight::getControlledLinks(id);
            for (size_t link_index = 0; link_index < controled_links.size();
                 link_index++) {
                for (auto link : controled_links[link_index]) {
                    size++;
                    from_lanes.emplace_back(link.fromLane);
                    via_lanes.emplace_back(link.viaLane);
                    to_lanes.emplace_back(link.toLane);
                    link_trafficlights.emplace_back(id);
                    link_indexs.emplace_back(link_index);
                }
            }
        }
    } catch (const std::exception& e) {
        return report_exception(e);
    }
    links = (Link*)malloc(size * sizeof(Link));
    if (links == NULL) return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
    for (size_t i = 0; i < size; i++) {
        links[i].from_lane = strdup(from_lanes[i].c_str());
        links[i].via_lane = strdup(via_lanes[i].c_str());
        links[i].to_lane = strdup(to_lanes[i].c_str());
        links[i].link_index = link_indexs[i];
        links[i].trafficlight = strdup(link_trafficlights[i].c_str());
        if (links[i].from_lane == NULL || links[i].via_lane == NULL ||
            links[i].to_lane == NULL || links[i].trafficlight == NULL) {
            if (links[i].from_lane != NULL) free(links[i].from_lane);
            if (links[i].via_lane != NULL) free(links[i].via_lane);
            if (links[i].to_lane != NULL) free(links[i].to_lane);
            if (links[i].trafficlight != NULL) free(links[i].trafficlight);
            for (size_t j = 0; j < i; j++) {
                free(links[j].from_lane);
                free(links[j].via_lane);
                free(links[j].to_lane);
                free(links[j].trafficlight);
            }
            free(links);
            return Payload{BRIDGE_MALLOC_FAILED, 0, 0};
        }
    }
    return Payload{BRIDGE_SUCCESS, (void*)links, size};
}
