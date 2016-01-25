<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="fr" lang="fr">
<head>
<title>Bons comptes</title>
<meta charset="utf-8"/>
<style>
body
{
  background-image: url("images/billets.jpg");
  background-position: center top;
}

p
{
  background-color:#eeeeff;
  opacity:1;
  filter:alpha(opacity=100); /* For IE8 and earlier */
}

table
{
  border-collapse:collapse;
  background-color:#ffffff;
  opacity:0.75;
  filter:alpha(opacity=75); /* For IE8 and earlier */
}

th,td
{
  border: 1px solid black;
}

h1
{
  background-color:#fefeff;
  opacity:1;
  filter:alpha(opacity=100); /* For IE8 and earlier */
}

h2
{
  background-color:#eeeeff;
  opacity:0.75;
  filter:alpha(opacity=75); /* For IE8 and earlier */
}
</style>
</head>
<body>
<?php
	include_once('./functions.php');
	$nodebt = db_connect();

	echo("<h1>Bons comptes</h1>\n");
	$user = get_string('user');

	echo("<h2>Actions:</h2>\n");
	$raw_user = get_raw_string('user');
	echo("<p><a href='./insert.php?user=$raw_user'>\n");
	echo("Je viens d'effectuer un paiement, d'autres devraient contribuer\n</a><br />\n");
	echo("<a href='./insert.php?user=$raw_user&contrib_num=0'>\n");
	echo("Je viens de rembourser quelqu'un\n</a><br />\n");
	echo("<a href='./insert.php?receiver=$raw_user'>\n");
	echo("Je viens de recevoir de l'argent, d'autres devraient être remboursés\n</a></p>\n");

	echo("<p><a href='./show?user=$raw_user'>\n");
	echo("Moins de détails\n</a></p>\n");

	print_table("Vous doit de l'argent&nbsp;:", $nodebt, "
			SELECT * FROM
			(
				SELECT fname, uname, ROUND(SUM(debt),2) AS debt
				FROM money_summary
				WHERE viewer_uname = '$user'
				AND user IS NOT NULL
				GROUP BY user
				ORDER BY debt DESC, fname
			) debt
			WHERE debt > 0
		",
		array("Personne", "Dette&nbsp;($)"),
		array("fname", "debt"),
		array("fname" => "uname")
	);

	print_table("Vous lui devez de l'argent&nbsp;:", $nodebt, "
			SELECT * FROM
			(
				SELECT fname, uname, ROUND(-SUM(debt),2) AS debt
				FROM money_summary
				WHERE viewer_uname = '$user'
				AND user IS NOT NULL
				GROUP BY user
				ORDER BY debt DESC, fname
			) debt
			WHERE debt > 0
		",
		array("Personne", "Dette&nbsp;($)"),
		array("fname", "debt"),
		array("fname" => "uname")
	);

	print_table("Paiements effectués&nbsp;:", $nodebt, "
			SELECT DATE(date) AS date, description, ROUND(money,2) AS money
			FROM money_out
			WHERE payer_uname = '$user'
			AND receiver IS NULL
			ORDER BY date DESC, money DESC
		",
		array("Date", "Description", "Montant&nbsp;($)"),
		array("date", "description", "money")
	);

	print_table("Argent reçue&nbsp;:", $nodebt, "
			SELECT DATE(date) AS date, description, ROUND(money,2) AS money
			FROM money_in
			WHERE receiver_uname = '$user'
			AND payer IS NULL
			ORDER BY date DESC, money DESC
		",
		array("Date", "Description", "Montant&nbsp;($)"),
		array("date", "description", "money")
	);

	print_table("Remboursements demandés&nbsp;:", $nodebt, "
			SELECT payer_fname, payer_uname, DATE(date) AS date, description, ROUND(money,2) AS money
			FROM money_asked
			WHERE receiver_uname = '$user'
			ORDER BY date DESC, money DESC
		",
		array("Personne", "Date", "Description", "Montant&nbsp;($)"),
		array("payer_fname", "date", "description", "money"),
		array("payer_fname" => "payer_uname")
	);

	print_table("Remboursements reçus&nbsp;:", $nodebt, "
			SELECT payer_uname, payer_fname, DATE(date) AS date, description, ROUND(money,2) AS money
			FROM money_in
			WHERE receiver_uname = '$user'
			AND payer IS NOT NULL
			ORDER BY date DESC, money DESC
		",
		array("Personne", "Date", "Description", "Montant&nbsp;($)"),
		array("payer_fname", "date", "description", "money"),
		array("payer_fname" => "payer_uname")
	);

	print_table("Remboursements envoyés&nbsp;:", $nodebt, "
			SELECT receiver_uname, receiver_fname, DATE(date) AS date, description, ROUND(money,2) AS money
			FROM money_in
			WHERE payer_uname = '$user'
			ORDER BY date DESC, money DESC
		",
		array("Personne", "Date", "Description", "Montant&nbsp;($)"),
		array("receiver_fname", "date", "description", "money"),
		array("receiver_fname" => "receiver_uname")
	);

	print_table("Demandes de remboursement reçues&nbsp;:", $nodebt, "
			SELECT receiver_fname, receiver_uname, DATE(date) AS date, description, ROUND(money,2) AS money
			FROM money_asked
			WHERE payer_uname = '$user'
			ORDER BY date DESC, money DESC
		",
		array("Personne", "Date", "Description", "Montant&nbsp;($)"),
		array("receiver_fname", "date", "description", "money"),
		array("receiver_fname" => "receiver_uname")
	);

	print_table("Détail&nbsp;:", $nodebt, "
			SELECT fname, uname, DATE(date) AS date, description, ROUND(-debt,2) AS debt
			FROM money_summary
			WHERE viewer_uname = '$user'
			AND user IS NOT NULL
			ORDER BY date DESC, fname
		",
		array("Personne", "Date", "Description", "Montant&nbsp;($)"),
		array("fname", "date", "description", "debt"),
		array("fname" => "uname")
	);

	$nodebt->close();

?>
</body>
</html>
