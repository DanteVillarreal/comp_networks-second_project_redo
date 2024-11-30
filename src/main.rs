//Name: Dante Villarreal
//ID: 1001580411
//student id: dxv0411

use std::collections::{VecDeque, HashMap};
use rand::Rng;
use plotters::prelude::*;

#[derive(Debug)]
enum EventType {
    Arrival { flow_id: u32, seq_num: u32 },
    Departure { flow_id: u32, seq_num: u32 },
    AckArrival { flow_id: u32, seq_num: u32 },
}

#[derive(Debug)]
struct Event {
    time: f64,
    event_type: EventType,
}

struct TCPFlow {
    rtt: f64,                // Round trip time
    cwnd: f64,              // Congestion window
    ssthresh: f64,          // Slow start threshold
    next_seq_num: u32,      // Next sequence number to send
    base_seq_num: u32,      // Oldest unacknowledged packet (Go-Back-N)
    expected_seq_num: u32,  // Next expected sequence number
    window_start: u32,      // Start of current window
    unacked_packets: HashMap<u32, f64>,  // seq_num -> send_time
    dup_ack_count: HashMap<u32, u32>,    // seq_num -> count
    total_sent: u32,
    total_received: u32,
    cumulative_throughput: f64,
    cumulative_goodput: f64,
    measurement_start_time: f64,
}

impl TCPFlow {
    fn new(rtt: f64) -> Self {
        TCPFlow {
            rtt,
            cwnd: 1.0,
            ssthresh: 16.0,
            next_seq_num: 0,
            base_seq_num: 0,
            expected_seq_num: 0,
            window_start: 0,
            unacked_packets: HashMap::new(),
            dup_ack_count: HashMap::new(),
            total_sent: 0,
            total_received: 0,
            cumulative_throughput: 0.0,
            cumulative_goodput: 0.0,
            measurement_start_time: 0.0,
        }
    }

    fn additive_increase(&mut self) {
        self.cwnd += 1.0 / self.cwnd;
    }

    fn multiplicative_decrease(&mut self) {
        self.ssthresh = self.cwnd / 2.0;
        self.cwnd = self.ssthresh;
    }

    fn update_metrics(&mut self, current_time: f64) {
        let time_delta = current_time - self.measurement_start_time;
        if time_delta > 0.0 {
            self.cumulative_throughput = self.total_sent as f64 / time_delta;
            self.cumulative_goodput = self.total_received as f64 / time_delta;
        }
    }

    fn handle_ack(&mut self, seq_num: u32) -> bool {
        if seq_num >= self.base_seq_num && seq_num < self.next_seq_num {
            if seq_num == self.expected_seq_num {
                // In-order ACK
                self.expected_seq_num += 1;
                self.base_seq_num = seq_num + 1;
                self.dup_ack_count.clear();
                self.additive_increase();
                self.total_received += 1;
                
                // Remove acknowledged packets
                for seq in self.window_start..=seq_num {
                    self.unacked_packets.remove(&seq);
                }
                self.window_start = seq_num + 1;
                true
            } else {
                // Duplicate ACK
                let count = self.dup_ack_count.entry(seq_num).or_insert(0);
                *count += 1;
                
                if *count == 3 {
                    // Triple duplicate ACK - Go-Back-N
                    self.multiplicative_decrease();
                    self.next_seq_num = self.base_seq_num; // Retransmit from base
                    true
                } else {
                    false
                }
            }
        } else {
            false
        }
    }

    fn can_send(&self) -> bool {
        (self.next_seq_num - self.base_seq_num) < self.cwnd.floor() as u32
    }
}

#[derive(Clone)]
struct FlowStatistics {
    time_points: Vec<f64>,
    cwnd_points: Vec<f64>,
    throughput_points: Vec<f64>,
    goodput_points: Vec<f64>,
    dropped_packets: u32,
}

impl FlowStatistics {
    fn new() -> Self {
        FlowStatistics {
            time_points: Vec::new(),
            cwnd_points: Vec::new(),
            throughput_points: Vec::new(),
            goodput_points: Vec::new(),
            dropped_packets: 0,
        }
    }

    fn record_point(&mut self, time: f64, cwnd: f64, throughput: f64, goodput: f64) {
        self.time_points.push(time);
        self.cwnd_points.push(cwnd);
        self.throughput_points.push(throughput);
        self.goodput_points.push(goodput);
    }
}

struct Simulation {
    current_time: f64,
    event_queue: VecDeque<Event>,
    buffer: VecDeque<(u32, u32)>,
    buffer_size: usize,
    service_rate: f64,
    flows: Vec<TCPFlow>,
    server_busy: bool,
    stats: Vec<FlowStatistics>,
    sample_interval: f64,
    next_sample_time: f64,
}

impl Simulation {
    fn new(buffer_size: usize, service_rate: f64, rtts: Vec<f64>, sample_interval: f64) -> Self {
        let num_flows = rtts.len();
        Simulation {
            current_time: 0.0,
            event_queue: VecDeque::new(),
            buffer: VecDeque::with_capacity(buffer_size),
            buffer_size,
            service_rate,
            flows: rtts.into_iter().map(TCPFlow::new).collect(),
            server_busy: false,
            stats: vec![FlowStatistics::new(); num_flows],
            sample_interval,
            next_sample_time: 0.0,
        }
    }

    fn record_stats(&mut self) {
        if self.current_time >= self.next_sample_time {
            for (i, flow) in self.flows.iter_mut().enumerate() {
                flow.update_metrics(self.current_time);
                self.stats[i].record_point(
                    self.current_time,
                    flow.cwnd,
                    flow.cumulative_throughput,
                    flow.cumulative_goodput
                );
            }
            self.next_sample_time = self.current_time + self.sample_interval;
        }
    }

    fn generate_service_time(&self) -> f64 {
        let z: f64 = rand::thread_rng().gen();
        -((1.0 - z).ln()) / self.service_rate
    }

    fn create_plots(&self, prefix: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Plot throughput over time
        self.plot_metric(prefix, "throughput", "Throughput (packets/sec)", 
                        |stats| &stats.throughput_points)?;
        
        // Plot goodput over time
        self.plot_metric(prefix, "goodput", "Goodput (packets/sec)", 
                        |stats| &stats.goodput_points)?;
        
        // Plot congestion window over time
        self.plot_metric(prefix, "cwnd", "Congestion Window Size", 
                        |stats| &stats.cwnd_points)?;

        Ok(())
    }

    fn plot_metric<F>(&self, prefix: &str, metric: &str, ylabel: &str, 
                      get_data: F) -> Result<(), Box<dyn std::error::Error>> 
    where F: Fn(&FlowStatistics) -> &Vec<f64> 
    {
        let filename = format!("{}_{}.png", prefix, metric);
        let root = BitMapBackend::new(&filename, (800, 600))
            .into_drawing_area();
        
        root.fill(&WHITE)?;

        let max_y = self.stats.iter()
            .flat_map(|s| get_data(s))
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        let max_x = self.stats[0].time_points.last()
            .copied()
            .unwrap_or(0.0);

        let mut chart = ChartBuilder::on(&root)
            .caption(format!("{} over Time", ylabel), ("sans-serif", 30))
            .margin(5)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(0f64..max_x, 0f64..max_y * 1.1)?;

        chart.configure_mesh()
            .x_desc("Time (seconds)")
            .y_desc(ylabel)
            .draw()?;

        let colors = [&RED, &BLUE];
        for (i, stats) in self.stats.iter().enumerate() {
            let points: Vec<(f64, f64)> = stats.time_points.iter()
                .zip(get_data(stats))
                .map(|(&x, &y)| (x, y))
                .collect();

            chart
                .draw_series(LineSeries::new(points, colors[i].clone()))?
                .label(format!("Flow {}", i))
                .legend(move |(x, y)| 
                    PathElement::new(vec![(x, y), (x + 20, y)], colors[i].clone()));
        }

        chart.configure_series_labels()
            .background_style(WHITE.mix(0.8))
            .border_style(BLACK)
            .draw()?;

        Ok(())
    }

    fn print_results(&self) {
        for (i, (flow, stats)) in self.flows.iter().zip(self.stats.iter()).enumerate() {
            println!("\nFlow {} Results:", i);
            println!("RTT: {:.3} seconds", flow.rtt);
            println!("Final cwnd: {:.3}", flow.cwnd);
            println!("Total packets sent: {}", flow.total_sent);
            println!("Total packets received: {}", flow.total_received);
            println!("Packets dropped: {}", stats.dropped_packets);
            println!("Drop rate: {:.2}%", 
                    100.0 * stats.dropped_packets as f64 / flow.total_sent as f64);
            println!("Average throughput: {:.3} packets/sec", flow.cumulative_throughput);
            println!("Average goodput: {:.3} packets/sec", flow.cumulative_goodput);
            println!("Efficiency: {:.2}%", 
                    100.0 * flow.cumulative_goodput / flow.cumulative_throughput);
        }
    }

    fn schedule_next_packet(&mut self, flow_id: u32) {
        let flow = &mut self.flows[flow_id as usize];
        
        while flow.can_send() {
            let event = Event {
                time: self.current_time,
                event_type: EventType::Arrival { 
                    flow_id,
                    seq_num: flow.next_seq_num,
                },
            };
            flow.next_seq_num += 1;
            self.event_queue.push_back(event);
        }
    }

    fn handle_arrival(&mut self, flow_id: u32, seq_num: u32) {
        let flow = &mut self.flows[flow_id as usize];
        flow.total_sent += 1;
        
        if self.buffer.len() < self.buffer_size {
            self.buffer.push_back((flow_id, seq_num));
            flow.unacked_packets.insert(seq_num, self.current_time);
            
            if !self.server_busy {
                self.start_service();
            }
        } else {
            // Record dropped packet
            self.stats[flow_id as usize].dropped_packets += 1;
            println!("Packet dropped: flow {} seq {} (total drops: {})", 
                    flow_id, seq_num, self.stats[flow_id as usize].dropped_packets);
        }

        self.schedule_next_packet(flow_id);
        self.record_stats();
    }

    fn handle_departure(&mut self, flow_id: u32, seq_num: u32) {
        // Schedule ACK arrival after RTT
        let rtt = self.flows[flow_id as usize].rtt;
        let ack_event = Event {
            time: self.current_time + rtt,
            event_type: EventType::AckArrival { flow_id, seq_num },
        };
        self.event_queue.push_back(ack_event);

        // Start serving next packet if any
        if !self.buffer.is_empty() {
            self.start_service();
        } else {
            self.server_busy = false;
        }
    }

    fn start_service(&mut self) {
        if let Some((flow_id, seq_num)) = self.buffer.pop_front() {
            self.server_busy = true;
            let service_time = self.generate_service_time();
            let departure_event = Event {
                time: self.current_time + service_time,
                event_type: EventType::Departure { flow_id, seq_num },
            };
            self.event_queue.push_back(departure_event);
        }
    }

    fn handle_ack(&mut self, flow_id: u32, seq_num: u32) {
        let flow = &mut self.flows[flow_id as usize];
        
        if flow.handle_ack(seq_num) {
            flow.update_metrics(self.current_time);
            // Schedule next packet if within window
            self.schedule_next_packet(flow_id);
        }
    }

    fn run(&mut self, simulation_time: f64) {
        // Initialize first packet for each flow
        for flow_id in 0..self.flows.len() {
            let flow = &mut self.flows[flow_id];
            flow.measurement_start_time = self.current_time;
            self.schedule_next_packet(flow_id as u32);
        }

        while self.current_time < simulation_time {
            // Sort events by time
            self.event_queue.make_contiguous();
            let mut events: Vec<_> = self.event_queue.drain(..).collect();
            events.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
            self.event_queue.extend(events);

            if let Some(event) = self.event_queue.pop_front() {
                self.current_time = event.time;
                
                match event.event_type {
                    EventType::Arrival { flow_id, seq_num } => 
                        self.handle_arrival(flow_id, seq_num),
                    EventType::Departure { flow_id, seq_num } => 
                        self.handle_departure(flow_id, seq_num),
                    EventType::AckArrival { flow_id, seq_num } =>
                        self.handle_ack(flow_id, seq_num),
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sample_interval = 0.1; // Record stats every 0.1 seconds
    let simulation_time = 20.0;
    let buffer_size = 50;
    let service_rate = 10.0;  // Increased from 1.0 to 10.0 packets/sec

    println!("TCP AIMD Simulation with Go-Back-N");
    println!("===================================");
    println!("Simulation parameters:");
    println!("  Buffer size: {} packets", buffer_size);
    println!("  Service rate: {} packets/sec", service_rate);
    println!("  Simulation time: {} seconds", simulation_time);
    println!("  Sample interval: {} seconds", sample_interval);

    // Scenario 1: Equal RTTs
    println!("\nScenario 1: Equal RTTs (0.1s, 0.1s)");
    println!("-----------------------------------");
    let mut sim1 = Simulation::new(buffer_size, service_rate, vec![0.1, 0.1], sample_interval);
    sim1.run(simulation_time);
    sim1.print_results();
    sim1.create_plots("equal_rtt")?;
    println!("Plots saved as equal_rtt_[metric].png");

    // Scenario 2: Different RTTs
    println!("\nScenario 2: Different RTTs (0.1s, 0.2s)");
    println!("---------------------------------------");
    let mut sim2 = Simulation::new(buffer_size, service_rate, vec![0.1, 0.2], sample_interval);
    sim2.run(simulation_time);
    sim2.print_results();
    sim2.create_plots("different_rtt")?;
    println!("Plots saved as different_rtt_[metric].png");

    println!("\nSimulation complete. Generated plots:");
    println!("- Throughput over time");
    println!("- Goodput over time");
    println!("- Congestion window size over time");

    Ok(())
}