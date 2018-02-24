extern crate winit;

use std::thread;
use std::time::{Duration,Instant};
use std::sync::mpsc;
use std::collections::VecDeque;

enum Action {
    WakeupSent(Instant),
    AwakenedReceived(Instant),
}

fn calculate_latency(rx: mpsc::Receiver<Action>) {
    thread::spawn(move || {
        let mut wakeups_sent: VecDeque<Instant> = VecDeque::new();
        let mut awakeneds_received: VecDeque<Instant> = VecDeque::new();

        let mut latency_history: Vec<u64> = Vec::with_capacity(1000);

        println!("wakeup() -> Event::Awakened latency (all times in µs)");
        println!("mean\tmax\t99%\t95%\t50%\t5%\t1%\tmin");

        while let Ok(action) = rx.recv() {
            match action {
                Action::WakeupSent(instant) => wakeups_sent.push_back(instant),
                Action::AwakenedReceived(instant) => awakeneds_received.push_back(instant),
            }

            while wakeups_sent.len() > 0 && awakeneds_received.len() > 0 {
                let sent = wakeups_sent.pop_front().unwrap();
                let recvd = awakeneds_received.pop_front().unwrap();
                if recvd > sent {
                    let latency = recvd.duration_since(sent);
                    let latency_us = latency.as_secs() * 1_000_000
                        + (latency.subsec_nanos() / 1_000) as u64;
                    latency_history.push(latency_us);
                }
            }

            if latency_history.len() > 300 {
                latency_history.sort();

                {
                    let mean = latency_history.iter()
                        .fold(0u64, |acc,&u| acc + u) / latency_history.len() as u64;
                    let max = latency_history.last().unwrap();
                    let pct99 = latency_history.get(latency_history.len() * 99 / 100).unwrap();
                    let pct95 = latency_history.get(latency_history.len() * 95 / 100).unwrap();
                    let pct50 = latency_history.get(latency_history.len() * 50 / 100).unwrap();
                    let pct5 = latency_history.get(latency_history.len() * 5 / 100).unwrap();
                    let pct1 = latency_history.get(latency_history.len() * 1 / 100).unwrap();
                    let min = latency_history.first().unwrap();
                    println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", mean, max, pct99, pct95, pct50, pct5, pct1, min);
                }

                latency_history.clear();
            }
        }
    });
}

fn send_wakeups(tx: mpsc::Sender<Action>, proxy: winit::EventsLoopProxy) {
    thread::spawn(move || {
        loop {
            let sent_at = Instant::now();
            proxy.wakeup().expect("wakeup");
            tx.send(Action::WakeupSent(sent_at)).unwrap();

            thread::sleep(Duration::from_secs(1) / 60);
        }
    });
}

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    let (tx,rx) = mpsc::channel::<Action>();

    calculate_latency(rx);
    send_wakeups(tx.clone(), events_loop.create_proxy());

    events_loop.run_forever(|event| {
        match event {
            winit::Event::Awakened { .. } => {
                // got awakened
                tx.send(Action::AwakenedReceived(Instant::now())).unwrap();

                winit::ControlFlow::Continue
            }

            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => {
                winit::ControlFlow::Break
            },
            _ => winit::ControlFlow::Continue,
        }
    });
}
