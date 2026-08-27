import trafficrs


class Node(trafficrs.NodeBase):
    def __init__(self, id):
        super().__init__()

    def on_tick(self, time):
        return

    def on_message(self, time_send, time_receive, sender, content):
        return

    def on_create(self):
        print(f"node {self.node_id} created")
        return


class Cloud(trafficrs.CloudBase):
    def __init__(self):
        super().__init__()

    def on_tick(self, time):
        return

    def on_message(self, time_send, time_receive, sender, content):
        return

    def on_create(self):
        return
