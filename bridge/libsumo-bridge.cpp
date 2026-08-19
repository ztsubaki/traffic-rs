#include "libsumo-bridge.h"

#include <cxxabi.h>
#include <libsumo/libsumo.h>

#include <cstring>

#include "libsumo/Simulation.h"
#include "libsumo/Vehicle.h"

using namespace libsumo;

const char* demangle(const char* name) {
    int status = 0;

    const char* p = abi::__cxa_demangle(name, nullptr, nullptr, &status);

    if (status) {
        char* s = strdup("<DEMANGLE FAILED>");
        if (!s) s = (char*)1;  // have very very very low bug risk
        return s;
    } else {
        return p;
    }
}

const void* load(const char* cfg) {
    try {
        Simulation::load({"-c", cfg, "--threads", "2"});
    } catch (const std::exception& e) {
        return (const void*)demangle(typeid(e).name());
    }
    return 0;
}

const void* step() {
    try {
        Simulation::step();
    } catch (const std::exception& e) {
        return (const void*)demangle(typeid(e).name());
    }
    return 0;
}

const void* close() {
    try {
        Simulation::close();
    } catch (const std::exception& e) {
        return (const void*)demangle(typeid(e).name());
    }
    return 0;
}

payload get_parameter(char* obj, char* para) {
    const char* result = NULL;
    try {
        std::string s = Simulation::getParameter(obj, para);
        result = strdup(s.c_str());
        if (result == NULL) {
            return payload{1, 0};
        }
    } catch (const std::exception& e) {
        return payload{(size_t)demangle(typeid(e).name()), 0};
    }
    return payload{0, (size_t)result};
}

payload get_option(char* opt) {
    const char* result = NULL;
    try {
        std::string s = Simulation::getOption(opt);
        result = strdup(s.c_str());
        if (result == NULL) {
            return payload{1, 0};
        }
    } catch (const std::exception& e) {
        return payload{(size_t)demangle(typeid(e).name()), 0};
    }
    return payload{0, (size_t)result};
}

payload vehicle_get_id_list() {
    char** vehicles = NULL;
    try {
        auto vehicles_vec = Vehicle::getIDList();
        vehicles = (char**)malloc((vehicles_vec.size() + 1) * sizeof(char*));
        if (vehicles == NULL) return payload{1, 0};
        for (size_t i = 0; i < vehicles_vec.size(); i++) {
            vehicles[i] = strdup(vehicles_vec[i].c_str());
            if (vehicles[i] == NULL) return payload{1, 0};
        }
        vehicles[vehicles_vec.size()] = NULL;
    } catch (const std::exception& e) {
        return payload{(size_t)demangle(typeid(e).name()), 0};
    }
    return payload{0, (size_t)vehicles};
}
