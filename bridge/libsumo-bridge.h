#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif
struct payload {
    size_t status;
    size_t payload;
};
const void* load(const char* cfg);
const void* step();
const void* close();
payload get_parameter(char* str);
payload get_option(char* opt);
payload vehicle_get_id_list();
#ifdef __cplusplus
}
#endif
