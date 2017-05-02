extern crate petgraph;
extern crate num;

use petgraph::algo;
use petgraph::data::FromElements;
use petgraph::dot::{Dot, Config};

extern crate clap;
use clap::{Arg, App};

use std::{iter, ops, default};
use std::vec::Vec;
use std::collections::HashSet;
use std::io::{self, BufReader};
use std::io::prelude::*;
use std::fs::File;
use std::process::Command;

type Number = Vec<i8>;
type Graph = petgraph::Graph<Number, i8, petgraph::Undirected>;
type EdgeIndex = petgraph::graph::EdgeIndex;
type NodeIndex = petgraph::graph::NodeIndex;

fn str_to_number(string: &str) -> io::Result<Number> {
    let mut result = Number::new();
    for ch in string.chars() {
        if let Some(number) = ch.to_digit(16) {
            result.push(number as i8);
        } else {
            let message = format!("{} is not a valid hex sequence", string);
            let error = io::Error::new(io::ErrorKind::Other, message);
            return Err(error);
        }
    }
    Ok(result)
}

fn number_to_str(number: &Number) -> String {
    number.iter().map(|x| x.to_string()).collect::<Vec<String>>().concat()
}

fn get_data(dot_file: &str) -> io::Result<Vec<Number>> {
    let file = BufReader::new(File::open(dot_file)?);
    let mut result = Vec::new();
    for line in file.lines() {
        result.push(str_to_number(&line?)?);
    }
    Ok(result)
}


fn manhattan_distance<It, Value, SubValue>(lhs: It, rhs: It) -> SubValue
    where It: iter::Iterator<Item = Value>,
          Value: ops::Sub<Output = SubValue>,
          SubValue: num::Signed + default::Default
{
    lhs.zip(rhs).fold(Default::default(), |acc, (x, y)| acc + (x - y).abs())
}

fn build_mst_graph(objects: &Vec<Number>) -> Graph {
    let mut result = petgraph::Graph::new_undirected();
    for obj in objects {
        result.add_node(obj.clone());
    }
    for lhs in objects.iter().enumerate() {
        for rhs in objects.iter().enumerate().skip(lhs.0 + 1) {
            let dist = manhattan_distance(lhs.1.iter(), rhs.1.iter());
            result.add_edge(NodeIndex::new(lhs.0), NodeIndex::new(rhs.0), dist);
        }
    }
    petgraph::Graph::from_elements(algo::min_spanning_tree(&result))
}

fn find_n_max_edges(graph: &Graph, number: usize) -> Vec<EdgeIndex> {
    // edges in MST are already sorted. Kind of a dirty hack I should not rely on...
    graph.edge_indices()
        .rev()
        .take(number)
        .collect()
}

fn remove_biggest_edges(graph: &Graph, clusters_number: usize) -> Graph {
    let mut graph_clone = graph.clone();
    for edge in find_n_max_edges(graph, clusters_number - 1) {
        graph_clone.remove_edge(edge);
    }
    graph_clone
}

fn deep_first_search(graph: &Graph, set: &mut HashSet<NodeIndex>) -> Vec<Number> {
    let mut stack = Vec::new();
    stack.push(set.iter().nth(0).unwrap().clone());
    let mut new_graph = Vec::new();
    while !stack.is_empty() {
        let node_index = stack.pop().unwrap().clone();
        if set.contains(&node_index) {
            set.remove(&node_index);
            new_graph.push(graph.node_weight(node_index).unwrap().clone());
            for node in graph.neighbors(node_index) {
                stack.push(node);
            }
        }
    }
    new_graph
}

fn find_connected_graphs(graph: &Graph) -> Vec<Vec<Number>> {
    let mut nodes: HashSet<_> = graph.node_indices().collect();
    let mut graphs = Vec::new();
    while !nodes.is_empty() {
        graphs.push(deep_first_search(&graph, &mut nodes));
    }
    graphs
}

fn graph_to_dot_file(graph: &Graph, dot_file: &str) -> io::Result<()> {
    let mut file = File::create(dot_file)?;
    let dot_header = r#"graph {
    graph [ fontname = "Helvetica", fontsize = 14, size = "500,500",
            splines=true, overlap=false, ratio=.5 ];
    node [ shape = plaintext, fontname = "Helvetica" ];"#;
    writeln!(&mut file, "{}", dot_header)?;
    for line in format!("{:?}", Dot::with_config(&graph, &[Config::NodeIndexLabel]))
        .lines()
        .skip(1) {
        writeln!(&mut file, "{}", line)?;
    }

    Ok(())
}

fn create_midpoint_element(cluster: &Vec<Number>) -> Number {
    cluster.iter()
        .fold(vec![0i64; cluster[0].len()], |result, number| {
            result.iter()
                .zip(number.iter())
                .map(|(r, n)| r + (n.clone() as i64))
                .collect()
        })
        .iter()
        .map(|v| (v.clone() as f64 / cluster.len() as f64).round() as i8)
        .collect()
}

fn output_midpoint_elements(clusters: &Vec<Vec<Number>>) {
    println!("Midpoint elements:");
    for (index, cl) in clusters.iter().enumerate() {
        println!("{} {}", number_to_str(&create_midpoint_element(&cl)), index);
    }
}

fn get_cluster_of(element: &Number, clusters: &Vec<Vec<Number>>) -> usize {
    for cl in clusters.iter().enumerate() {
        match cl.1.iter().find(|&x| x == element) {
            Some(_) => return cl.0,
            None => (),
        }
    }
    panic!("Some shit happened");
}

fn clusters_to_file(numbers: &Vec<Number>,
                    clusters: &Vec<Vec<Number>>,
                    filename: &str)
                    -> io::Result<()> {
    let mut file = File::create(filename)?;
    for number in numbers {
        writeln!(&mut file,
                 "{} {}",
                 number_to_str(&number),
                 get_cluster_of(&number, &clusters))?;
    }
    Ok(())
}

fn get_args<'a>() -> clap::ArgMatches<'a> {
    App::new("MST Clasterization Program")
        .version("1.0")
        .author("Vlad Alex <vlad.al.dp@gmail.com>")
        .about("Performs clusterization of records in file using MST graph clusterization \
                algorithm")
        .arg(Arg::with_name("INPUT")
            .help("Sets the input file to use")
            .required(true)
            .index(1))
        .arg(Arg::with_name("CLUSTERS")
            .help("Sets the number of clusters")
            .required(true)
            .index(2))
        .arg(Arg::with_name("output")
            .short("o")
            .long("output")
            .value_name("FILE")
            .help("Sets a custom output file")
            .takes_value(true))
        .arg(Arg::with_name("dot")
            .short("d")
            .long("dot")
            .value_name("FILE")
            .help("Sets a custom dot output file")
            .takes_value(true))
        .arg(Arg::with_name("graph-image")
            .short("g")
            .long("graph-image")
            .value_name("FILE")
            .help("Sets a custom graph image output file")
            .takes_value(true))
        .get_matches()
}

fn main() {
    let args = get_args();
    let infile = args.value_of("INPUT").unwrap();
    println!("Reading data from {}...", infile);
    let data = match get_data(infile) {
        Ok(data) => data,
        Err(message) => {
            println!("Error reading data: {}", message);
            std::process::exit(1);
        }
    };
    println!("Building graph...");
    let mst_graph = build_mst_graph(&data);
    println!("Detecting clusters...");
    let clusters_number = match args.value_of("CLUSTERS").unwrap().parse() {
        Ok(value) => value,
        Err(message) => {
            println!("Invalid number of clusters: {}", message);
            std::process::exit(1);
        }
    };

    let clusters_graph = remove_biggest_edges(&mst_graph, clusters_number);
    let dot_file = args.value_of("dot").unwrap_or("graph.dot");
    if args.is_present("dot") || args.is_present("graph-image") {
        match graph_to_dot_file(&clusters_graph, dot_file) {
            Ok(_) => (),
            Err(message) => {
                println!("Error writing dot file: {}", message);
                return;
            }
        }
    }

    if args.is_present("graph-image") {
        let imagefile = args.value_of("graph-image").unwrap();
        println!("Creating graph image {}...", imagefile);
        let ecode = match Command::new("twopi")
            .arg("-Tjpg")
            .arg(dot_file)
            .arg("-o")
            .arg(imagefile)
            .output() {
            Ok(output) => output.status,
            Err(message) => {
                println!("Failed to execute twopi: {}", message);
                return;
            }
        };
        if !ecode.success() {
            println!("Failed to create graph image");
        }
    }

    let clusters = find_connected_graphs(&clusters_graph);
    output_midpoint_elements(&clusters);
    let outfile = args.value_of("output").unwrap_or("outfile.txt");
    println!("Writing output to {}", outfile);
    match clusters_to_file(&data, &clusters, outfile) {
        Ok(_) => (),
        Err(message) => {
            println!("Error writing output file: {}", message);
            return;
        }
    }
}
