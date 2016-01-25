<?php
	include_once("sql.php");
	$nodebt = db_connect();
	if(!empty($_POST["do_insert"]))
	{
	
	}
	else
	{
?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="fr" lang="fr">
<head>
<title>Ajout d'un utilisateur &ndash; Bons comptes</title>
<meta charset="utf-8"/>
</head>
<body>
<?php
	}
	$nodebt->close();
?>
