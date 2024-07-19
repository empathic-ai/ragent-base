use bevy::utils::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref VOICE_ID_BY_NAME: HashMap<String, String> = {
        let mut map = HashMap::new();
        map.insert("default".to_string(), "XIe6oC3VvU9SJSpmCMRD".to_string());
        map.insert("host".to_string(), "Yko7PKHZNXotIFUBG7I9".to_string());
        //map.insert("narrator".to_string(), "XIe6oC3VvU9SJSpmCMRD".to_string());
        map.insert("kind-man-a".to_string(), "9F4C8ztpNUmXkdDDbz3J".to_string());
        map.insert("kind-man-b".to_string(), "uN4FTvnHwmukUeSiYxVq".to_string());
        map.insert("rough-man".to_string(), "N2lVS1w4EtoT3dr4eOWO".to_string());
        map.insert("kind-girl-a".to_string(), "pN7g4tQyXKBDawJSw7Q8".to_string());
        map.insert("kind-girl-b".to_string(), "W2wCwtpOQyVoOrSZKSpo".to_string());
        map.insert("soft-man".to_string(), "MuoHnmlnSXfhGXY2o3im".to_string());
        map.insert("old-woman".to_string(), "GamzzeOsatMbpvYvKdVx".to_string());
        map.insert("deep-man".to_string(), "5rnzAtDpoJXVGo8Td3mO".to_string());
        map.insert("raspy-man".to_string(), "t0jbNlBVZ17f02VDIeMI".to_string());
        //map.insert("smexy-frog".to_string(), "TJ5lH6wsCvZ2alDg7xrH".to_string());
        // Old smexy frog voice, more gritty
        map.insert("man-a".to_string(), "N2lVS1w4EtoT3dr4eOWO".to_string());
        // Anatra voice
        map.insert("child-a".to_string(),"gkvpzrzujkR7osK9HAgg".to_string());
        // Baby chick voice
        map.insert("child-b".to_string(), "Z8aiHDBl5U16BuihS7ss".to_string());
        // Bearsworth voice
        map.insert("silly-man".to_string(), "542jzeOaLKbcpZhWfJDa".to_string());
		map.insert("anna".to_string(), "q6bboItSc3laqmM0fge1".to_string());
        // Old anatra voice
        //map.insert("anatra".to_string(), "F9UvutjrG9l2yvDB2zSt".to_string());
        map
    };
}