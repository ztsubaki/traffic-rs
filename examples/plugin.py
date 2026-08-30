"""Comprehensive traffic-rs Python API example.

The example accesses every read-only network object and V2X record property.
It prints once for each Node at startup, once when the Cloud starts, and one
Cloud acknowledgement for Node 0. It does not print from on_tick or for every
message.
"""

import trafficrs


class Node(trafficrs.NodeBase):
    def __init__(self, node_id, net):
        super().__init__()

    def on_create(self):
        traffic_light = self.net.get_trafficlight(self.id)
        phase = next(
            (phase for program in traffic_light.phases for phase in program),
            None,
        )
        link = next(
            (link for alternatives in traffic_light.links for link in alternatives),
            None,
        )

        summary = {
            "node_id": self.id,
            "traffic_light_id": traffic_light.id,
            "link_groups": len(traffic_light.links),
            "phase_programs": len(traffic_light.phases),
            "phase_state": phase.sigs if phase else None,
            "phase_next": phase.next if phase else [],
        }

        if link:
            from_lane_id = getattr(link, "from")
            from_lane = self.net.get_lane(from_lane_id)
            via_lane = self.net.get_lane(link.via) if link.via is not None else None
            to_lane = self.net.get_lane(link.to)
            summary["link"] = {
                "from": from_lane_id,
                "via": link.via,
                "to": link.to,
            }
            summary["lanes"] = {
                "from": self._lane_summary(from_lane),
                "via": self._lane_summary(via_lane),
                "to": self._lane_summary(to_lane),
            }

        print(f"Node initialized: {summary}")

        # Demonstrate Node-to-Cloud messaging once per Node, not per tick.
        self._sent_rsu_summary = False
        self._printed_acknowledgement = False
        self.send_to_cloud({"kind": "node_ready", "node_id": self.id})

        # Demonstrate a valid initial traffic-light action.
        self.set_signal("r" * len(traffic_light.links))

    def on_tick(self, time):
        # Intentionally silent: this callback is invoked every simulation step.
        pass

    def on_message(self, time_sent, time_received, sender, content):
        if sender is None:
            # Cloud-to-Node messages use sender=None. Print once globally enough
            # to show the complete round trip without flooding the console.
            if self.id == 0 and not self._printed_acknowledgement:
                print(
                    "Node 0 received Cloud acknowledgement: "
                    f"sent={time_sent}ms received={time_received}ms content={content}"
                )
                self._printed_acknowledgement = True
            return

        # RSU-to-Node content is {vehicle_id: Record}. Summarize only the first
        # report received by each Node to avoid forwarding every RSU update.
        if self._sent_rsu_summary:
            return

        reports = list(content.values())
        vehicle_snapshot = None
        record_timing = None
        if reports:
            record = reports[0]
            vehicle = record.vehicle
            vehicle_snapshot = {
                "lane": vehicle.lane,
                "lane_position": vehicle.lane_position,
                "speed": vehicle.speed,
                "acceleration": vehicle.acceleration,
                "waiting_time": vehicle.waiting_time,
                "time_loss": vehicle.time_loss,
                "next_trafficlight": vehicle.next_trafficlight,
                "next_link_index": vehicle.next_link_index,
            }
            record_timing = {
                "time_send": record.time_send,
                "time_arrive": record.time_arrive,
            }

        self.send_to_cloud(
            {
                "kind": "rsu_summary",
                "node_id": self.id,
                "rsu_id": sender,
                "report_count": len(reports),
                "sample_vehicle": vehicle_snapshot,
                "sample_record_timing": record_timing,
                "node_received_at": time_received,
            }
        )
        self._sent_rsu_summary = True

    def _lane_summary(self, lane):
        if lane is None:
            return None

        edge = self.net.get_edge(lane.edge_id) if lane.edge_id is not None else None
        edge_from = getattr(edge, "from") if edge is not None else None
        from_junction = (
            self.net.get_junction(edge_from)
            if edge_from is not None
            else None
        )
        to_junction = (
            self.net.get_junction(edge.to)
            if edge is not None and edge.to is not None
            else None
        )
        return {
            "id": lane.id,
            "edge_id": lane.edge_id,
            "length": lane.length,
            "outgoing_links": len(lane.links),
            "traffic_lights": sorted(lane.trafficlight),
            "edge": None
            if edge is None
            else {
                "id": edge.id,
                "name": edge.name,
                "from": edge_from,
                "to": edge.to,
                "lane_number": edge.lane_number,
            },
            "junctions": {
                "from": from_junction.id if from_junction else None,
                "to": to_junction.id if to_junction else None,
            },
        }


class Cloud(trafficrs.CloudBase):
    def __init__(self, net):
        super().__init__()

    def on_create(self):
        # The Cloud has the same read-only PyNet object as every Node.
        assert self.net is not None
        trafficrs.hello_world()
        print("Cloud controller initialized")

    def on_tick(self, time):
        # Intentionally silent: this callback is invoked every simulation step.
        pass

    def on_message(self, time_sent, time_received, sender, content):
        # Node-to-Cloud messages provide the sender Node ID. Reply without
        # printing each message so large simulations do not flood stdout.
        self.send_to_node(
            sender,
            {
                "status": "received",
                "kind": content["kind"],
                "node_sent_at": time_sent,
                "cloud_received_at": time_received,
            },
        )
