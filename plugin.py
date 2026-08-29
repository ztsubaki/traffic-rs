import trafficrs


class Node(trafficrs.NodeBase):
    def __init__(self, node_id, net):
        super().__init__()

    def on_tick(self, time):
        #        if time % 1000000 == 0:
        #            print(f"Node {self.node_id} tick {time}")
        #            self.send_to_cloud("hello")
        if time == 0:
            sign = "r" * len(self.net.get_trafficlight(self.node_id).links)
            self.set_signal(sign)
        return

    def on_message(self, time_send, time_receive, sender, content):
        # print(f'Node {self.node_id} received message from {sender} send {time_send} received {time_receive}')
        return

    def on_create(self):
        print(
            f"Node {self.node_id} created on {self.net.get_trafficlight(self.node_id).id}"
        )
        return


class Cloud(trafficrs.CloudBase):
    def __init__(self, net):
        super().__init__()

    def on_tick(self, time):
        return

    def on_message(self, time_send, time_receive, sender, content):
        #        print(f"Cloud received message from {sender} send {
        #              time_send} received {time_receive}: {content}")
        return

    def on_create(self):
        return
