import cytoscape from "./cytoscape.esm.min.mjs";
import cola from "https://esm.sh/cytoscape-cola"
import yaml from "https://esm.sh/js-yaml";

cytoscape.use(cola);
export let cy = cytoscape({
        container: document.getElementById("vis"),
        style: [
            {
                selector: 'node',
                style: {
                    'label': 'data(label)',
                    'text-halign': 'center',
                    'text-valign': 'center',
                    'color': 'black'
                }
            },
            {
                selector: 'edge',
                style: {
                    'label': 'data(weight)',
                    'line-color': 'data(color)',
                    'arrow-scale': '1',
                    'curve-style': 'bezier'
                }
            },
            {
                selector: '.ok',
                style: {
                    'line-color': '#87CEEB'
                }
            },
            {
                selector: '.infeasible',
                style: {
                    'line-color': 'orange',
                    "background-color": 'orange'
                }
            },
            {
                selector: '.fake',
                style: {
                    'line-color': 'red'
                }
            },
            {
                selector: '.forward',
                style: {
                    'target-arrow-shape': 'triangle'
                }
            },
            {
                selector: '.backward',
                style: {
                    'source-arrow-shape': 'triangle'
                }
            },
            {
                selector: ':selected',
                style: {
                    'background-color': '#87CEEB'
                }
            },
        ]
    }
)

// default layout options
var defaults = {
    animate: true, // whether to show the layout as it's running
    refresh: 1, // number of ticks per frame; higher is faster but more jerky
    // maxSimulationTime: 4000, // max length in ms to run the layout
    infinite: true,

    ungrabifyWhileSimulating: false, // so you can't drag nodes during layout
    fit: false, // on every layout reposition of nodes, fit the viewport
    padding: 30, // padding around the simulation
    boundingBox: {
        x1: 0,
        y1: 0,
        w: 1000,
        h: 1000
    }, // constrain layout bounds; { x1, y1, x2, y2 } or { x1, y1, w, h }
    nodeDimensionsIncludeLabels: false, // whether labels should be included in determining the space used by a node

    // layout event callbacks
    ready: function () {
    }, // on layoutready
    stop: function () {
    }, // on layoutstop

    // positioning options
    randomize: false, // use random node positions at beginning of layout
    avoidOverlap: true, // if true, prevents overlap of node bounding boxes
    handleDisconnected: true, // if true, avoids disconnected components from overlapping
    convergenceThreshold: 0.01, // when the alpha value (system energy) falls below this value, the layout stops
    nodeSpacing: function (node) {
        return 5;
    }, // extra spacing around nodes
    flow: undefined, // use DAG/tree flow layout if specified, e.g. { axis: 'y', minSeparation: 30 }
    alignment: undefined, // relative alignment constraints on nodes, e.g. {vertical: [[{node: node1, offset: 0}, {node: node2, offset: 5}]], horizontal: [[{node: node3}, {node: node4}], [{node: node5}, {node: node6}]]}
    gapInequalities: undefined, // list of inequality constraints for the gap between the nodes, e.g. [{"axis":"y", "left":node1, "right":node2, "gap":25}]
    centerGraph: false, // adjusts the node positions initially to center the graph (pass false if you want to start the layout from the current position)

    // different methods of specifying edge length
    // each can be a constant numerical value or a function like `function( edge ){ return 2; }`
    edgeLength: 200, // sets edge length directly in simulation
    edgeSymDiffLength: undefined, // symmetric diff edge length in simulation
    edgeJaccardLength: undefined, // jaccard edge length in simulation

    // iterations of cola algorithm; uses default values on undefined
    unconstrIter: undefined, // unconstrained initial layout iterations
    userConstIter: undefined, // initial layout iterations with user-specified constraints
    allConstIter: undefined, // initial layout iterations with all constraints including non-overlap
};

console.log(cy)

function orderedPair(a, b) {
    if (a < b) {
        return a + "-" + b
    } else {
        return b + "-" + a
    }
}

function orderedPairTuple(a, b) {
    if (a < b) {
        return {x:a,y:b}
    } else {
        return {x:b,y:a}
    }
}

function parseEdge(edge){
    let x = edge.split(" ")
    let a = parseInt(x[0]);
    let b = parseInt(x[1]);
    let c = parseInt(x[2]);
    return {a,b,c}
}

function parseRoute(route){
    let x = route.split(" ")
    return {
        src: parseInt(x[0]),
        nextHop: parseInt(x[1]),
        seq: parseInt(x[2]),
        metric: parseInt(x[3]),
        feasibility: parseInt(x[4])
    }
}

function getEdgeDirection(a,b){
    let {x,y} = orderedPairTuple(a,b);
    if(x === a){
        return "forward"
    }
    return "backward"
}

let curData = null;
let selected = null;
let highlighting = false;
function highlight(){
    highlighting = true;
    for(let ele of cy.mutableElements()){
        if(ele.id() !== selected)
            ele.deselect()

        ele.removeClass("forward")
        ele.removeClass("backward")
        ele.removeClass("infeasible")
        ele.removeClass("fake")
        ele.removeClass("ok")
    }
    recalcGraph(curData)
    if(selected == null) return;
    let nodeId = parseInt(selected.substring(1,2));
    console.log(nodeId)
    let feasible = new Set();
    let infeasible = new Set();
    let fake = new Set();
    for(let route of curData["routes"][nodeId]){
        let {src, nextHop, seq, metric, feasibility} = parseRoute(route)
        if(metric === 65535)
            infeasible.add("n" + src);
    }

    for (let node in curData["nodes"]) {
        let dstId = parseInt(node);
        if(dstId === nodeId){
            continue;
        }
        console.log(`To: ${node}`)

        let path = getNodePath(nodeId, dstId, 0);
        console.log(path)
        for(let i = 0; i < path.length - 1; i++){
            let a = path[i][0];
            let b = path[i+1][0];
            let aGood = path[i][1] !== 65535;
            let bGood = path[i+1][1] !== 65535;
            let edgeId = "e" + orderedPair(a,b);
            if(aGood && bGood){
                feasible.add(edgeId);
            }
            else{
                infeasible.add(edgeId);
            }
            if(!cy.hasElementWithId(edgeId)){
                fake.add(edgeId);
                infeasible.add("n" + a);
                infeasible.add("n" + b);
                let {x,y} = orderedPairTuple(a,b);
                cy.add({
                    group: 'edges',
                    data: {
                        id: edgeId,
                        source: "n" + x,
                        target: "n" + y,
                        weight: "inf",
                        color: "white"
                    }
                })
            }
            let ele = cy.getElementById(edgeId)
            ele.addClass(getEdgeDirection(a,b))
        }
    }
    for(let ele of cy.mutableElements()){
        ele.removeClass("infeasible")
        ele.removeClass("fake")
        ele.removeClass("ok")
        if(infeasible.has(ele.id())){
            ele.addClass("infeasible")
        }
        if(feasible.has(ele.id())){
            ele.removeClass("infeasible")
            ele.addClass("ok")
        }
        if(fake.has(ele.id())){
            ele.removeClass("infeasible")
            ele.addClass("fake")
        }
    }

    highlighting = false;
}

function getNodePath(from, dst, cost) {
    if(from === dst){
        return [[dst, cost]]
    }
    console.log(`${from}->${dst} c=${cost}`)
    for(let route of curData["routes"][from]){
        if(!route.endsWith("self")){
            let parsed = parseRoute(route);
            let {src, nextHop, seq, metric, feasibility} = parsed
            // console.log(parsed)
            if(src === dst){
                // console.log("matched!")
                let nc = (metric === 65535 || cost === 65535) ? 65535 : (cost + metric);
                return [[from, cost], ...getNodePath(nextHop, dst, nc)]
            }
        }
    }
    return []
}

let seqno = document.querySelector('#seqno')

cy.on('tap', 'node', function(evt){
    selected = evt.target.id();
    highlight()
    seqno.style.display = 'block'
});

cy.on('unselect', function(evt){
    if(highlighting) return;
    let tgt = evt.target.id();
    if(selected === tgt){
        selected = null;
        seqno.style.display = 'none'
        console.log("unselected all nodes")
    }
});

seqno.addEventListener('click', async () => {
    if(!curData["actions"]){
        curData["actions"] = {}
    }
    if(!curData["actions"]["req"]){
        curData["actions"]["req"] = []
    }
    curData["actions"]["req"].push(
        selected.substring(1,2)
    )
    window.setEditorText(yaml.dump(curData))
})

export function updateGraph(data){
    recalcGraph(data)
    cy.centre()
    highlight()
}

export function recalcGraph(data) {
    curData = data;
    let ids = new Set()
    for (let node in data["nodes"]) {
        let id = "n" + node;
        ids.add(id)
        if (!cy.hasElementWithId(id)) {
            cy.add({
                group: 'nodes',
                data: {
                    id: id,
                    label: node
                }
            })
        }
    }
    for (let edge of data["neighbours"]) {
        let {a,b,c} = parseEdge(edge)
        let id = "e" + orderedPair(a, b)
        ids.add(id)
        if (!cy.hasElementWithId(id)) {
            let {x,y} = orderedPairTuple(a,b);
            cy.add({
                group: 'edges',
                data: {
                    id: id,
                    source: "n" + x,
                    target: "n" + y,
                    weight: c,
                    color: "white"
                }
            })
        }
        else{
            cy.getElementById(id).data("weight", c)
        }
    }

    for(let ele of cy.elements()){
        let id = ele.id()
        if(!ids.has(id)){
            cy.remove(ele)
        }
    }

    // cy.centre()
    if(!window.cola){
        let options = {
            name: 'cola',
            ...defaults
        };

        window.cola = cy.layout(options);
        window.cola.run()
    }
}

export async function runSim(text) {
    const resp = await fetch("/sim_route", {
        method: "POST",
        body: text
    })
    let res = await resp.text();
    if (resp.ok) {
        return res
    } else {
        let lines = text.split("\n");
        if (lines.length == 0 || lines[0].startsWith("#")) {
            lines[0] = "# [Error]: " + res;
        } else {
            lines = ["# [Error]: " + res, ...lines]
        }
        return lines.join("\n")
    }
}