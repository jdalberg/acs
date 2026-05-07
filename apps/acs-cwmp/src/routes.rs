use crate::{handlers, models::EventMessage};
use warp::Filter;

// pub so we can call it from tests
pub fn inform_route(
    event_tx: tokio::sync::mpsc::Sender<EventMessage>,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let tx_filter = warp::any().map(move || event_tx.clone());
    // Define the route for the inform endpoint - should be XML content
    warp::path!("acs")
        .and(warp::post())
        .and(warp::body::bytes())
        .and(tx_filter)
        .and_then(handlers::inform)
}

// pub so we can call it from tests
pub fn hello_route(
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("hello").and_then(handlers::hello)
}

pub fn build_routes(
    event_tx: tokio::sync::mpsc::Sender<EventMessage>,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    // Combine all routes using `.or()` to create a set of routes
    inform_route(event_tx).or(hello_route())
}

#[cfg(test)]
mod tests {
    use warp::test;

    use crate::routes::{hello_route, inform_route};

    #[tokio::test]
    async fn test_hello_route() {
        let res = test::request()
            .method("GET")
            .path("/hello")
            .reply(&hello_route())
            .await;

        assert_eq!(res.status(), 200);
        assert_eq!(res.body(), "Hello, world!");
    }

    #[tokio::test]
    async fn test_inform_route() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);
        let sample = include_bytes!("../tests/samples/good_inform_1.xml");

        let res = test::request()
            .method("POST")
            .path("/acs")
            .body(&sample)
            .reply(&inform_route(event_tx))
            .await;

        assert_eq!(res.status(), 200); // Verify that the handler sent the correct message over the channel.
        if let Some(event) = event_rx.recv().await {
            let events = event.events.unwrap();
            assert_eq!(events.len(), 1);
        } else {
            panic!("Expected to receive an event message, but got None");
        }
    }
}
