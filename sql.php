<?php
#do not print anything in this file, those functions are called before header.

	function db_string($val)
	{
		return htmlentities($val, ENT_QUOTES, "UTF-8");
	}

	function db_number($val)
	{
		if(is_numeric($val))
			return $val;
		//maybe the input is in french
		$val = str_replace(",", ".", $val);
		$val = str_replace(" ", "", $val);
		if(is_numeric($val))
			return $val;
		return 0;
	}

	function db_date($val)
	{
		if(empty($val) || $val == "NOW" || $val == "MAINTENANT")
		{
			date_default_timezone_set('America/Montreal');
			return date("Y-m-d H:i:s");
		}
		return db_string($val);
	}

	function get_string($name)
	{
		if(isset($_REQUEST[$name]))
			return db_string($_REQUEST[$name]);
		return NULL;
	}

	function get_raw_string($name)
	{
		if(isset($_REQUEST[$name]))
			return $_REQUEST[$name];
		return NULL;
	}

	function get_number($name)
	{
		if(isset($_REQUEST[$name]))
			return db_number($_REQUEST[$name]);
		return NULL;
	}

	function get_date($name)
	{
		if(isset($_REQUEST[$name]))
			return db_date($_REQUEST[$name]);
		return date("Y-m-d H:i:s");
	}

	function db_user()
	{
		if($uid = get_number('uid'))
			return $uid;
		elseif($user = get_string('user'))
		{
			$sql = "SELECT id FROM user WHERE uname = '$user'";
			$sql_result = $mysqli->query($sql);
			if(!$sql_result)
				die($mysqli->error);
			if($sql_result->num_rows == 0)
				$row[0] = NULL;
			else
				$row = $sql_result->fetch_array(MYSQL_NUM);
			$sql_result->close();
			return $row[0];
		}
		return NULL;
	}

	function db_connect()
	{
		$nodebt = new mysqli("localhost","nodebt","SCZ3QtWFcuDAWR79","nodebt");
		if ($nodebt->connect_error)
		{
			die('Erreur de connexion (' . $nodebt->connect_errno . ') '
				    . $mysqli->connect_error);
		}

		if(!$nodebt->set_charset("utf8"))
		{
			$nodebt->close();
			printf("<!-- Error loading character set utf8: %s -->\n",
				$mysqli->error);
		}

		return $nodebt;
	}
?>
